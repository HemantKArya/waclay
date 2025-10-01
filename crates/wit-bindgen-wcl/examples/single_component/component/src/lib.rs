wit_bindgen::generate!({
    path: "wit/component.wit",
    exports: {
        "test:guest/foo": Foo
    }
});

struct Foo;

impl exports::test::guest::foo::Guest for Foo {
    fn select_nth(x: Vec<String>, n: u32) -> String {
        x.into_iter()
            .nth(n as usize)
            .unwrap_or_else(|| format!("Index {} out of bounds", n))
    }
}
