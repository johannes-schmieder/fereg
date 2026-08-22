fn main() {
    let answer = rust_smoke::add(20, 22);
    println!("Rust smoke test answer: {answer}");
    assert_eq!(answer, 42);
}
