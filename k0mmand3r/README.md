k0mmand3r is a rust library a string parser for "/slash" commands

the library will be packaged as ALL OF THE ABOVE:
1. k0mmand3r a rust crate for other rust programs
    * https://crates.io/crates/k0mmand3r
2. k0mmand3r_ts wasm using wasmer/bindgen (for typescript) k0mmand3r
    * gated by #[cfg(target_arch = "wasm32")]
    * because wasm-pack doesn't support feature flags!
3. k0mmand3r_py python using maturin/pyo3 (a rust framework)
    * gated by #[cfg(feature = "lang-python")]

the k0mmand3r library core should attempt to be "DRY" (don't repeat yourself) and avoid implementing any duplicative business parsing logic using good idiomatic composition.

As such the core parsing logic is in Rust, but since it uses lifetimes & generics those aren't compatible with wasm/python.

The wasm version converts to the Result json string before output.
The python version converts to an equivalent python class.

there should be _mostly_ complete test coverage.

the parse command will follow these rules:
    parse can receive commands or content or both

any string should first be trimmed for whitespace on the front and back.
    after trimming, if found the kommand must begin with a forward slash "/",
    any string which doesn't begin with a / is returned as "content" (it has no command)

if a kommand is found then parse proceeds to parse the grammar of the command
here are the rules for parsing a kommand grammar:

zero or more parameters will be found
    --parameters are prefixed by a double dash "--"
    parameters are always alphanumeric
    if a parameter is followed by an = then it will have a value token
    so --parameter or --parameter=value can be returned
    the order of parameters is important and should be preserved in the structures
        a parameter with no value is called a "tag"
        a parameter with a value is a type "kvpair"
    values can be of four types:
        1. string
        2. number
        3. boolean
        4. @user  (a user token begins with a literal "@" followed by a letter, followed by one or more alphanumeric or emoji characters
        5. #channel (a channel token begins with a literal "#" followed by a letter, followed by one or more alphanumeric or emoji characters

    these structures once parsed should be stored in an k0mmand3r result object


