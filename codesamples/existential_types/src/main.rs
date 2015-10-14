use std::vec::Vec;

mod term;
mod func;

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

#[test]
fn term_test_1() {
    use func::Func;
    use func::Curry;
    use term::Term;
    let fun = |a: String, b: usize| {
        let mut v = Vec::new();
        for i in 0..b {
            v.push(format!("{}: {}", i, a));
        }
        v
    };
    let curry_term = term::Lift(Curry(fun));
    let apply_1 = term::Apply(curry_term, term::Lift(String::from("hello")));
    let apply_2 = term::Apply(apply_1, term::Lift(3));
    assert!(apply_2.run() == vec!["0: hello!", "1: hello!", "2: hello!"]);
}
