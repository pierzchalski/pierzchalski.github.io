#![feature(box_patterns, fnbox)]

use std::vec::Vec;

mod term;
mod func;
mod work;
mod cont;

fn main() {

}

#[test]
fn currying_test_1() {
    use func::Func;
    use func::Curry;
    let fun = |a: String, b: usize| {
        let mut v = Vec::new();
        for i in 0..b {
            v.push(format!("{}: {}", i, a));
        }
        v
    };
    let curry = Curry(fun);
    let curry1 = curry.call(String::from("hello!"));
    let result = curry1.call(3);
    assert!(result == vec!["0: hello!", "1: hello!", "2: hello!"]);
}
