initSidebarItems({"fn":[["and_then",""],["any","Parses any token"],["between","Parses `open` followed by `parser` followed by `close` Returns the value of `parser`"],["chainl1","Parses `p` 1 or more times separated by `op` The value returned is the one produced by the left associative application of `op`"],["chainr1","Parses `p` one or more times separated by `op` The value returned is the one produced by the right associative application of `op`"],["choice","Takes an array of parsers and tries to apply them each in order. Fails if all parsers fails or if an applied parser consumes input before failing."],["count","Parses up to `count` times using `parser`."],["env_parser","Constructs a parser out of an environment and a function which needs the given environment to do the parsing. This is commonly useful to allow multiple parsers to share some environment while still allowing the parsers to be written in separate functions."],["eof","Succeeds only if the stream is at end of input, fails otherwise."],["expected",""],["flat_map",""],["look_ahead","look_ahead acts as p but doesn't consume input on success."],["many","Parses `p` zero or more times returning a collection with the values from `p`. If the returned collection cannot be inferred type annotations must be supplied, either by annotating the resulting type binding `let collection: Vec<_> = ...` or by specializing when calling many, `many::<Vec<_>, _>(...)`"],["many1","Parses `p` one or more times returning a collection with the values from `p`. If the returned collection cannot be inferred type annotations must be supplied, either by annotating the resulting type binding `let collection: Vec<_> = ...` or by specializing when calling many1 `many1::<Vec<_>, _>(...)`"],["map",""],["message",""],["none_of","Extract one token and succeeds if it is not part of `tokens`."],["not_followed_by","Succeeds only if `parser` fails. Never consumes any input."],["one_of","Extract one token and succeeds if it is part of `tokens`."],["optional","Returns `Some(value)` and `None` on parse failure (always succeeds)"],["or",""],["parser","Wraps a function, turning it into a parser Mainly needed to turn closures into parsers as function types can be casted to function pointers to make them usable as a parser"],["position","Parser which just returns the current position in the stream"],["satisfy","Parses a token and succeeds depending on the result of `predicate`"],["satisfy_map","Parses a token and passes it to `predicate`. If `predicate` returns `Some` the parser succeeds and returns the value inside the `Option`. If `predicate` returns `None` the parser fails without consuming any input."],["sep_by","Parses `parser` zero or more time separated by `separator`, returning a collection with the values from `p`. If the returned collection cannot be inferred type annotations must be supplied, either by annotating the resulting type binding `let collection: Vec<_> = ...` or by specializing when calling sep_by, `sep_by::<Vec<_>, _, _>(...)`"],["sep_by1","Parses `parser` one or more time separated by `separator`, returning a collection with the values from `p`. If the returned collection cannot be inferred type annotations must be supplied, either by annotating the resulting type binding `let collection: Vec<_> = ...` or by specializing when calling sep_by, `sep_by1::<Vec<_>, _, _>(...)`"],["sep_end_by","Parses `parser` zero or more time separated by `separator`, returning a collection with the values from `p`. If the returned collection cannot be inferred type annotations must be supplied, either by annotating the resulting type binding `let collection: Vec<_> = ...` or by specializing when calling sep_by, `sep_by::<Vec<_>, _, _>(...)`"],["sep_end_by1","Parses `parser` one or more time separated by `separator`, returning a collection with the values from `p`. If the returned collection cannot be inferred type annotations must be supplied, either by annotating the resulting type binding `let collection: Vec<_> = ...` or by specializing when calling sep_by, `sep_by1::<Vec<_>, _, _>(...)`"],["skip",""],["skip_many","Parses `p` zero or more times ignoring the result"],["skip_many1","Parses `p` one or more times ignoring the result"],["then",""],["token","Parses a character and succeeds if the character is equal to `c`"],["tokens","Parses `tokens`."],["try","Try acts as `p` except it acts as if the parser hadn't consumed any input if `p` returns an error after consuming input"],["unexpected","Always fails with `message` as an unexpected error. Never consumes any input."],["value","Always returns the value `v` without consuming any input."],["with",""]],"struct":[["AndThen",""],["Any",""],["Between",""],["Chainl1",""],["Chainr1",""],["Choice",""],["Count",""],["EnvParser",""],["Eof",""],["Expected",""],["FlatMap",""],["FnParser",""],["Iter",""],["LookAhead",""],["Many",""],["Many1",""],["Map",""],["Message",""],["NoneOf",""],["NotFollowedBy",""],["OneOf",""],["Optional",""],["Or",""],["Position",""],["Satisfy",""],["SatisfyMap",""],["SepBy",""],["SepBy1",""],["SepEndBy",""],["SepEndBy1",""],["Skip",""],["SkipMany",""],["SkipMany1",""],["Then",""],["Token",""],["Tokens",""],["Try",""],["Unexpected",""],["Value",""],["With",""]]});