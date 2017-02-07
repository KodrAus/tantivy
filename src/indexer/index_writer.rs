use schema::Schema;
use schema::Document;
use super::operation::AddOperation;
use core::Index;
use core::Segment;
use core::SegmentId;
use schema::Term;
use indexer::SegmentEntry;
use std::thread::JoinHandle;
use indexer::MergePolicy;
use indexer::SegmentWriter;
use DocId;
use bit_set::BitSet;
use fastfield::delete::write_delete_bitset;
use postings::SegmentPostingsOption;
use postings::DocSet;
use core::SegmentComponent;
use super::directory_lock::DirectoryLock;
use futures::Future;
use std::clone::Clone;
use std::io;
use fastfield::delete;
use std::thread;
use futures::Canceled;
use std::mem;
use datastruct::stacker::Heap;
use core::SegmentReader;
use std::mem::swap;
use chan;
use core::SegmentMeta;
use super::delete_queue::{DeleteQueue, DeleteQueueCursor};
use super::segment_updater::SegmentUpdater;
use Result;
use Error;

// Size of the margin for the heap. A segment is closed when the remaining memory
// in the heap goes below MARGIN_IN_BYTES.
pub const MARGIN_IN_BYTES: u32 = 10_000_000u32;

// We impose the memory per thread to be at least 30 MB.
pub const HEAP_SIZE_LIMIT: u32 = MARGIN_IN_BYTES * 3u32;

// Add document will block if the number of docs waiting in the queue to be indexed reaches PIPELINE_MAX_SIZE_IN_DOCS
const PIPELINE_MAX_SIZE_IN_DOCS: usize = 10_000;



type DocumentSender = chan::Sender<AddOperation>;
type DocumentReceiver = chan::Receiver<AddOperation>;



/// `IndexWriter` is the user entry-point to add document to an index.
///
/// It manages a small number of indexing thread, as well as a shared
/// indexing queue.
/// Each indexing thread builds its own independant `Segment`, via
/// a `SegmentWriter` object.
pub struct IndexWriter {
    
    // the lock is just used to bind the 
    // lifetime of the lock with that of the IndexWriter.
    _directory_lock: DirectoryLock, 
    
    index: Index,

    heap_size_in_bytes_per_thread: usize,

    workers_join_handle: Vec<JoinHandle<Result<()>>>,

    document_receiver: DocumentReceiver,
    document_sender: DocumentSender,

    segment_updater: SegmentUpdater,

    worker_id: usize,

    num_threads: usize,

    generation: usize,

    delete_queue: DeleteQueue,

    uncommitted_opstamp: u64,
    committed_opstamp: u64,
}

// IndexWriter cannot be sent to another thread.
impl !Send for IndexWriter {}
impl !Sync for IndexWriter {}


pub enum DocToOpstampMapping {
    WithMap(Vec<u64>),
    None
}

impl DocToOpstampMapping { 
    fn compute_doc_limit(&self, opstamp: u64) -> DocId {
        match *self {
            DocToOpstampMapping::WithMap(ref doc_opstamps) => {
                match doc_opstamps.binary_search(&opstamp) {
                    Ok(doc_id) => doc_id as DocId,
                    Err(doc_id) => doc_id as DocId,
                }
            }
            DocToOpstampMapping::None => DocId::max_value(),
        }
    }
}



/// TODO
/// work on SegmentMeta
pub fn advance_deletes(
    segment: &Segment,
    delete_cursor: &mut DeleteQueueCursor,
    doc_opstamps: DocToOpstampMapping) -> Result<Option<(u64, BitSet)>> {
        let segment_reader = SegmentReader::open(segment.clone())?;
        let mut delete_bitset = BitSet::with_capacity(segment_reader.max_doc() as usize);

        let mut last_opstamp_opt: Option<u64> = None;
        for delete_op in delete_cursor {
            // A delete operation should only affect
            // document that were inserted after it.
            // 
            // Limit doc helps identify the first document
            // that may be affected by the delete operation.
            let limit_doc = doc_opstamps.compute_doc_limit(delete_op.opstamp);
            if let Some(mut docset) = segment_reader.read_postings(&delete_op.term, SegmentPostingsOption::NoFreq) {
                while docset.advance() {
                    let deleted_doc = docset.doc();
                    if deleted_doc < limit_doc {
                        delete_bitset.insert(deleted_doc as usize);
                    }
                }
                last_opstamp_opt = Some(delete_op.opstamp);
            }
        }

        if let Some(last_opstamp) = last_opstamp_opt {
            for doc in 0u32..segment_reader.max_doc() {
                if segment_reader.is_deleted(doc) {
                    delete_bitset.insert(doc as usize);
                }
            }
            Ok(Some((last_opstamp, delete_bitset)))
        }
        else {
            Ok(None)
        }
}

fn index_documents(heap: &mut Heap,
                   mut segment: Segment,
                   schema: &Schema,
                   generation: usize,
                   document_iterator: &mut Iterator<Item=AddOperation>,
                   segment_updater: &mut SegmentUpdater,
                   delete_cursor: &mut DeleteQueueCursor)
                   -> Result<bool> {
    heap.clear();
    let segment_id = segment.id();
    let mut segment_writer = try!(SegmentWriter::for_segment(heap, segment.clone(), &schema));
    for doc in document_iterator {
        try!(segment_writer.add_document(&doc, &schema));
        if segment_writer.is_buffer_full() {
            info!("Buffer limit reached, flushing segment with maxdoc={}.",
                  segment_writer.max_doc());
            break;
        }
    }
    let num_docs = segment_writer.max_doc();
    
    // this is ensured by the call to peek before starting
    // the worker thread.
    assert!(num_docs > 0);    
    
    segment
        .meta_mut()
        .set_num_docs(num_docs);

    let last_opstamp = segment_writer.last_opstamp();
    
    let doc_opstamps: Vec<u64> = segment_writer.finalize()?;

    if let Some((last_opstamp_after_deletes, deleted_docset)) = advance_deletes(&segment, delete_cursor, DocToOpstampMapping::WithMap(doc_opstamps))? {
        let num_deleted_docs = deleted_docset.len();
        segment.meta_mut().set_deletes(num_deleted_docs as u32, last_opstamp_after_deletes);
        let mut delete_file = segment.open_write(SegmentComponent::DELETE)?;
        write_delete_bitset(&deleted_docset, &mut delete_file)?;
    }

    let segment_entry = SegmentEntry::new(segment.meta().clone(), delete_cursor.clone());

    segment_updater
        .add_segment(generation, segment_entry)
        .wait()
        .map_err(|_| Error::ErrorInThread("Could not add segment.".to_string()))

}


impl IndexWriter {
    /// The index writer
    pub fn wait_merging_threads(mut self) -> Result<()> {
        
        // this will stop the indexing thread,
        // dropping the last reference to the segment_updater.
        drop(self.document_sender);
        
        let mut v = Vec::new();
        mem::swap(&mut v, &mut self.workers_join_handle);
        for join_handle in v {
            try!(join_handle.join()
                .expect("Indexing Worker thread panicked")
                .map_err(|e| {
                    Error::ErrorInThread(format!("Error in indexing worker thread. {:?}", e))
                }));
        }
        drop(self.workers_join_handle);

        self.segment_updater
            .wait_merging_thread()
            .map_err(|_| 
                Error::ErrorInThread("Failed to join merging thread.".to_string())
            )
    }

    /// Spawns a new worker thread for indexing.
    /// The thread consumes documents from the pipeline.
    ///
    fn add_indexing_worker(&mut self) -> Result<()> {
        let index = self.index.clone();
        let schema = self.index.schema();
        let document_receiver_clone = self.document_receiver.clone();
        let mut segment_updater = self.segment_updater.clone();
        let mut heap = Heap::with_capacity(self.heap_size_in_bytes_per_thread);
        
        // TODO fix this. the cursor might be too advanced
        // at this point.
        let delete_cursor = self.delete_queue.cursor();
        
        let generation = self.generation;

        let join_handle: JoinHandle<Result<()>> =
            thread::Builder::new()
            .name(format!("indexing thread {} for gen {}", self.worker_id, generation))
            .spawn(move || {
                
                let mut delete_cursor_clone = delete_cursor.clone();
                loop {
                    let mut document_iterator = document_receiver_clone.clone()
                        .into_iter()
                        .peekable();

                    // the peeking here is to avoid
                    // creating a new segment's files
                    // if no document are available.
                    //
                    // this is a valid guarantee as the 
                    // peeked document now belongs to
                    // our local iterator.
                    let opstamp: u64;
                    if let Some(operation) = document_iterator.peek() {
                        opstamp = operation.opstamp;
                    }
                    else {
                        // No more documents.
                        // Happens when there is a commit, or if the `IndexWriter`
                        // was dropped.
                        return Ok(())
                    }

                    let segment = index.new_segment();
                    let valid_generation = index_documents(&mut heap,
                                            segment,
                                            &schema,
                                            generation,
                                            &mut document_iterator,
                                            &mut segment_updater,
                                            &mut delete_cursor_clone)?;
                }
            })?;
        self.worker_id += 1;
        self.workers_join_handle.push(join_handle);
        Ok(())
    }


    /// Open a new index writer. Attempts to acquire a lockfile.
    ///
    /// The lockfile should be deleted on drop, but it is possible
    /// that due to a panic or other error, a stale lockfile will be
    /// left in the index directory. If you are sure that no other
    /// `IndexWriter` on the system is accessing the index directory,
    /// it is safe to manually delete the lockfile.
    ///
    /// num_threads specifies the number of indexing workers that
    /// should work at the same time.
    /// # Errors
    /// If the lockfile already exists, returns `Error::FileAlreadyExists`.
    /// # Panics
    /// If the heap size per thread is too small, panics.
    pub fn open(index: &Index,
                num_threads: usize,
                heap_size_in_bytes_per_thread: usize)
                -> Result<IndexWriter> {

        if heap_size_in_bytes_per_thread <= HEAP_SIZE_LIMIT as usize {
            panic!(format!("The heap size per thread needs to be at least {}.",
                           HEAP_SIZE_LIMIT));
        }
        
        let directory_lock = try!(DirectoryLock::lock(index.directory().box_clone()));
        
        let (document_sender, document_receiver): (DocumentSender, DocumentReceiver) =
            chan::sync(PIPELINE_MAX_SIZE_IN_DOCS);


        let delete_queue = DeleteQueue::default();
        
        let segment_updater = SegmentUpdater::new(index.clone(), delete_queue.cursor())?;
        
        let mut index_writer = IndexWriter {
            
            _directory_lock: directory_lock,
            
            heap_size_in_bytes_per_thread: heap_size_in_bytes_per_thread,
            index: index.clone(),

            document_receiver: document_receiver,
            document_sender: document_sender,

            segment_updater: segment_updater,

            workers_join_handle: Vec::new(),
            num_threads: num_threads,

            delete_queue: delete_queue,

            committed_opstamp: index.opstamp(),
            uncommitted_opstamp: index.opstamp(),

            generation: 0,

            worker_id: 0,
        };
        try!(index_writer.start_workers());
        Ok(index_writer)
    }
    

    pub fn get_merge_policy(&self) -> Box<MergePolicy> {
        self.segment_updater.get_merge_policy()
    }

    /// Set the merge policy.
    pub fn set_merge_policy(&self, merge_policy: Box<MergePolicy>) {
        self.segment_updater.set_merge_policy(merge_policy);
    }
    
    fn start_workers(&mut self) -> Result<()> {
        for _ in 0..self.num_threads {
            try!(self.add_indexing_worker());
        }
        Ok(())
    }

    /// Merges a given list of segments
    pub fn merge(&mut self, segment_ids: &[SegmentId]) -> impl Future<Item=SegmentEntry, Error=Canceled> {
        self.segment_updater.start_merge(segment_ids)
    }

    /// Closes the current document channel send.
    /// and replace all the channels by new ones.
    ///
    /// The current workers will keep on indexing
    /// the pending document and stop
    /// when no documents are remaining.
    ///
    /// Returns the former segment_ready channel.
    fn recreate_document_channel(&mut self) -> DocumentReceiver {
        let (mut document_sender, mut document_receiver): (DocumentSender, DocumentReceiver) =
            chan::sync(PIPELINE_MAX_SIZE_IN_DOCS);
        swap(&mut self.document_sender, &mut document_sender);
        swap(&mut self.document_receiver, &mut document_receiver);
        document_receiver
    }

    /// Rollback to the last commit
    ///
    /// This cancels all of the update that
    /// happened before after the last commit.
    /// After calling rollback, the index is in the same
    /// state as it was after the last commit.
    ///
    /// The opstamp at the last commit is returned.
    pub fn rollback(&mut self) -> Result<u64> {

        // by updating the generation in the segment updater,
        // pending add segment commands will be dismissed.
        self.generation += 1;
        let rollback_future = self.segment_updater.new_generation(self.generation);
        
        // we cannot drop segment ready receiver yet
        // as it would block the workers.
        let document_receiver = self.recreate_document_channel();

        // Drains the document receiver pipeline :
        // Workers don't need to index the pending documents.
        for _ in document_receiver {}

        let mut former_workers_join_handle = Vec::new();
        swap(&mut former_workers_join_handle,
             &mut self.workers_join_handle);
        
        // wait for all the worker to finish their work
        // (it should be fast since we consumed all pending documents)
        for worker_handle in former_workers_join_handle {
            // we stop one worker at a time ...
            try!(try!(worker_handle.join()
                .map_err(|e| Error::ErrorInThread(format!("{:?}", e)))));
            // ... and recreate a new one right away
            // to work on the next generation.
            try!(self.add_indexing_worker());
        }

        // All of our indexing workers for the rollbacked generation have
        // been terminated.
        // 
        // Our document receiver pipe was drained.
        // No new document have been added in the meanwhile because `IndexWriter`
        // is not shared by different threads.
        
        rollback_future.wait().map_err(|_|
            Error::ErrorInThread("Error while waiting for rollback.".to_string())
        )?;

        // reset the opstamp
        self.uncommitted_opstamp = self.committed_opstamp;
        Ok(self.committed_opstamp)
    }

    /// Commits all of the pending changes
    ///
    /// A call to commit blocks.
    /// After it returns, all of the document that
    /// were added since the last commit are published
    /// and persisted.
    ///
    /// In case of a crash or an hardware failure (as
    /// long as the hard disk is spared), it will be possible
    /// to resume indexing from this point.
    ///
    /// Commit returns the `opstamp` of the last document
    /// that made it in the commit.
    ///
    pub fn commit(&mut self) -> Result<u64> {

        // this will drop the current document channel
        // and recreate a new one channels.
        self.recreate_document_channel();

        let mut former_workers_join_handle = Vec::new();
        swap(&mut former_workers_join_handle,
             &mut self.workers_join_handle);
        
        for worker_handle in former_workers_join_handle {
            let indexing_worker_result = try!(worker_handle.join()
                .map_err(|e| Error::ErrorInThread(format!("{:?}", e))));
            try!(indexing_worker_result);
            // add a new worker for the next generation.
            try!(self.add_indexing_worker());
        }

        // here, because we join all of the worker threads,
        // all of the segment update for this commit have been
        // sent.
        //
        // No document belonging to the next generation have been
        // pushed too, because add_document can only happen
        // on this thread.

        // This will move uncommitted segments to the state of
        // committed segments.
        self.committed_opstamp = self.stamp();

        let future = self.segment_updater.commit(self.committed_opstamp);

        // wait for the segment update thread to have processed the info
        // TODO remove unwrap
        future.wait().unwrap();

        Ok(self.committed_opstamp)
    }    

    
    pub fn delete_term(&mut self, term: Term) {
        let opstamp = self.stamp();
        self.delete_queue.push(opstamp, term);
    }

    fn stamp(&mut self) -> u64 {
        let opstamp = self.uncommitted_opstamp;
        self.uncommitted_opstamp += 1u64;
        opstamp
    }

    /// Adds a document.
    ///
    /// If the indexing pipeline is full, this call may block.
    ///
    /// The opstamp is an increasing `u64` that can
    /// be used by the client to align commits with its own
    /// document queue.
    ///
    /// Currently it represents the number of documents that
    /// have been added since the creation of the index.
    pub fn add_document(&mut self, document: Document) -> io::Result<u64> {
        let opstamp = self.stamp();
        let add_operation = AddOperation {
            opstamp: opstamp,
            document: document,
        };
        self.document_sender.send(add_operation);
        Ok(opstamp)
    }
}




#[cfg(test)]
mod tests {

    use indexer::NoMergePolicy;
    use schema::{self, Document};
    use Index;
    use Term;
    use Error;

    #[test]
    fn test_lockfile_stops_duplicates() {
        let schema_builder = schema::SchemaBuilder::default();
        let index = Index::create_in_ram(schema_builder.build());
        let _index_writer = index.writer(40_000_000).unwrap();
        match index.writer(40_000_000) {
            Err(Error::FileAlreadyExists(_)) => {}
            _ => panic!("Expected FileAlreadyExists error"),
        }
    }
    
    #[test]
    fn test_set_merge_policy() {
        let schema_builder = schema::SchemaBuilder::default();
        let index = Index::create_in_ram(schema_builder.build());
        let index_writer = index.writer(40_000_000).unwrap();
        assert_eq!(format!("{:?}", index_writer.get_merge_policy()), "LogMergePolicy { min_merge_size: 8, min_layer_size: 10000, level_log_size: 0.75 }");
        let merge_policy = box NoMergePolicy::default();
        index_writer.set_merge_policy(merge_policy);
        assert_eq!(format!("{:?}", index_writer.get_merge_policy()), "NoMergePolicy");
    }

    #[test]
    fn test_lockfile_released_on_drop() {
        let schema_builder = schema::SchemaBuilder::default();
        let index = Index::create_in_ram(schema_builder.build());
        {
            let _index_writer = index.writer(40_000_000).unwrap();
            // the lock should be released when the 
            // index_writer leaves the scope.
        }
        let _index_writer_two = index.writer(40_000_000).unwrap();
    }
    
    #[test]
    fn test_commit_and_rollback() {
        let mut schema_builder = schema::SchemaBuilder::default();
        let text_field = schema_builder.add_text_field("text", schema::TEXT);
        let index = Index::create_in_ram(schema_builder.build());

        let num_docs_containing = |s: &str| {
            let searcher = index.searcher();
            let term_a = Term::from_field_text(text_field, s);
            searcher.doc_freq(&term_a)
        };

        {
            // writing the segment
            let mut index_writer = index.writer_with_num_threads(3, 40_000_000).unwrap();
            {
                let mut doc = Document::default();
                doc.add_text(text_field, "a");
                index_writer.add_document(doc).unwrap();
            }
            assert_eq!(index_writer.rollback().unwrap(), 0u64);
            assert_eq!(num_docs_containing("a"), 0);

            {
                let mut doc = Document::default();
                doc.add_text(text_field, "b");
                index_writer.add_document(doc).unwrap();
            }
            {
                let mut doc = Document::default();
                doc.add_text(text_field, "c");
                index_writer.add_document(doc).unwrap();
            }
            assert_eq!(index_writer.commit().unwrap(), 2u64);
            index.load_searchers().unwrap();
            assert_eq!(num_docs_containing("a"), 0);
            assert_eq!(num_docs_containing("b"), 1);
            assert_eq!(num_docs_containing("c"), 1);
        }
        index.load_searchers().unwrap();
        index.searcher();
    }

    #[test]
    fn test_with_merges() {
        let mut schema_builder = schema::SchemaBuilder::default();
        let text_field = schema_builder.add_text_field("text", schema::TEXT);
        let index = Index::create_in_ram(schema_builder.build());
        let num_docs_containing = |s: &str| {
            let searcher = index.searcher();
            let term_a = Term::from_field_text(text_field, s);
            searcher.doc_freq(&term_a)
        };
        {
            // writing the segment
            let mut index_writer = index.writer_with_num_threads(4, 4 * 30_000_000).unwrap();
            // create 10 segments with 100 tiny docs
            for _doc in 0..100 {
                let mut doc = Document::default();
                doc.add_text(text_field, "a");
                index_writer.add_document(doc).unwrap();
            }
            index_writer.commit().expect("commit failed");
            for _doc in 0..100 {
                let mut doc = Document::default();
                doc.add_text(text_field, "a");
                index_writer.add_document(doc).unwrap();
            }
            // this should create 8 segments and trigger a merge.
            index_writer.commit().expect("commit failed");
            index_writer.wait_merging_threads().expect("waiting merging thread failed");
            index.load_searchers().unwrap();
            assert_eq!(num_docs_containing("a"), 200);
            assert_eq!(index.searchable_segments().unwrap().len(), 1);
        }
    }


}
