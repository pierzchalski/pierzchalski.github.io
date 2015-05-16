---
layout:     post
title:      HKT in Rust, Today
date:       2015-05-16
summary:    Fun with kinds in Rust.
---

There's a pretty common pattern that people encounter all over the place when programming: if you have a container `C` holding a thing of type `A`, and you've got a function that turns `A`s into `B`s, you can get a `C` containing a `B`.
You can see an example in Pythons `for` comprehensions: if I have a list of strings, I can get a list of integers:

{% highlight python %} [ len(string) for string in strings ] {% endhighlight %}

There's something similar for Rusts vectors:[^1]
[^1]: This post was written using the Rust nightly version 

{% highlight rust %}
fn main() {
    let vec1: Vec<&str> = vec!["Hello", ", ", "World!"];
    let vec2: Vec<usize> = convert_vec(vec1, |string| {
        string.len()
    });
    println!("{:?}", vec2); // prints "[5, 2, 6]"
}

fn convert_vec<A, B, F>(vec: Vec<A>, fun: F) -> Vec<B>
    where F: Fn(A) -> B
{
    let mut vec_ret: Vec<B> = Vec::with_capacity(vec.len());
    for a in vec {
        vec_ret.push(fun(a));
    }
    vec_ret
}
{% endhighlight %}

There's also something similar for `Option<A>`, and for `Result<A, Err>`, and for pairs `(First, A)`, and (I think this one is kind of cool) for `Box<Fn(Start) -> A>`:

{% highlight rust %}
fn main() {
    let fun1 = Box::new(|string: &str| {string.len()});
    let fun2 = convert_fun(fun1, |len| {len * 3});
    println!("{:?}", fun2("Cool")); // prints "12"

    let fun3 = convert_fun(fun2, |number| {
        format!("I have a number: {}", number)
    });
    println!("{}", fun3("Cool")); // prints "I have a number: 12"
}

fn convert_fun<'a, Start, A, B, F>
    (fun: Box<Fn(Start) -> A + 'a>, f: F) -> 
        Box<Fn(Start) -> B + 'a>
    where F: Fn(A) -> B + 'a
{
    Box::new(move |start| {
        f(fun(start))
    })
}
{% endhighlight %}

