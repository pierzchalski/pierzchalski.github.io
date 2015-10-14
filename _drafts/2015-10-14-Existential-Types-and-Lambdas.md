---
layout: post
title: Curries and Existential Crises
date: 2015-10-14
summary: Like a Haskell tutorial, but with more implementation details!
---

So, you've found yourself a fancy new low-level systems language that promises
to expose all the low-level implementation details that need to be exposed,
while hiding all the ones that make people cry.

What's the first thing that you do (after screwing around with the FFI and
getting it completely wrong)? Implement
[function currying!](https://en.wikipedia.org/wiki/Currying)
Along the way we'll learn about existential types and how to represent them in
Rust.

### A First Attempt

Currying is pretty straightforward: we've got some function `f: Fn(A, B) -> C`
and we've got some argument `a: A`, so we want to just bundle them up together
and get `f(a): Fn(B) -> C`. Later on, when we've got some `b: B`, we want to
repeat the process, getting an `f(a)(b): C`.

Rust
[exposes](https://doc.rust-lang.org/std/ops/trait.Fn.html)
[enough](https://doc.rust-lang.org/std/ops/trait.FnMut.html)
[functionality](https://doc.rust-lang.org/std/ops/trait.FnOnce.html)
for us to get the above syntax natively, but it's probably not worth the effort
just yet. For the moment, we'll roll our own custom trait `Func`:

```rust
pub trait Func<Arg> {
    type Output;
    fn call(self, arg: Arg) -> Self::Output;
}
```

You might notice that this is pretty much a copy of Rust's
[FnOnce](https://doc.rust-lang.org/std/ops/trait.FnOnce.html).
I'm choosing that because for these purposes I find it easiest to reason about
functions that have a clear owner and which can be run exactly once.

So, we can clearly 'lift' functions into our fancy new trait (we could have
just implemented the trait for `FnOnce(A) -> B` directly, but going down
that road leads to some issues when we also implement it for `Fn`):

```rust
pub struct Lift<F>(pub F);

impl<F, A, B> Func<A> for Lift<F>
    where F: FnOnce(A) -> B
{
    type Output = B;
    fn call(self, arg: A) -> B {
        (self.0)(arg)
    }
}
```

And it looks like this is already enough to implement currying! We'll start
with another wrapping struct (so we can distinguish what we're going to
do with the function we're wrapping), along with the obvious intermediate
argument-storing struct:

```rust
pub struct Curry<F>(pub F);

pub struct Curry1<F, A>(pub F, pub A);

impl<F, A> Func<A> for Curry<F>
{
    type Output = Curry1<F, A>;
    fn call(self, arg: A) -> Curry1<F, A> {
        Curry1(self.0, arg)
    }
}

impl <F, A, B, C> Func<B> for Curry1<F, A>
    where F: FnOnce(A, B) -> C
{
    type Output = C;
    fn call(self, arg: B) -> C {
        (self.0)(self.1, arg)
    }
}
```

Let's check that it works (I've put my above code into a module called `func`):

```rust
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
```
```
$ cargo test
test currying_test_1 ... ok
```

Hooray! Well that was easy. Why don't we try something a little harder? Our
`Func<A>` trait tracks the argument type, but sometimes we only care about the
final result of a computation. Let's make a trait `Term` to encapsulate
this concept:

```rust
trait Term {
    type Result;
    fn run(self) -> Self::Result;
}
```
