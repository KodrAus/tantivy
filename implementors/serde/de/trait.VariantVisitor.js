(function() {var implementors = {};
implementors["bincode"] = ["impl&lt;'a, R:&nbsp;<a class='trait' href='https://doc.rust-lang.org/nightly/std/io/trait.Read.html' title='trait std::io::Read'>Read</a>&gt; <a class='trait' href='serde/de/trait.VariantVisitor.html' title='trait serde::de::VariantVisitor'>VariantVisitor</a> for <a class='struct' href='bincode/serde/struct.Deserializer.html' title='struct bincode::serde::Deserializer'>Deserializer</a>&lt;'a, R&gt;",];
implementors["serde"] = [];
implementors["tantivy"] = ["impl&lt;'a, R&gt; <a class='trait' href='serde/de/trait.VariantVisitor.html' title='trait serde::de::VariantVisitor'>VariantVisitor</a> for <a class='struct' href='bincode/serde/reader/struct.Deserializer.html' title='struct bincode::serde::reader::Deserializer'>Deserializer</a>&lt;'a, R&gt; <span class='where fmt-newline'>where R: <a class='trait' href='https://doc.rust-lang.org/nightly/std/io/trait.Read.html' title='trait std::io::Read'>Read</a></span>",];

            if (window.register_implementors) {
                window.register_implementors(implementors);
            } else {
                window.pending_implementors = implementors;
            }
        
})()
