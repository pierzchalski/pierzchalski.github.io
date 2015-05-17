---
layout:     post
title:      High Abstraction in the Land of Rust
date:       2015-05-16
summary:    Fun with kinds in Rust.
---

There's a pretty common pattern that people encounter all over the place when programming: if you have a container `C` holding a thing of type `A`, and you've got a function that turns `A`s into `B`s, you can get a `C` containing a `B`.
You can see an example in Python `for` comprehensions: if I have a list of strings, I can get a list of integers:

{% highlight python %} [ len(string) for string in strings ] {% endhighlight %}

There's something similar for Rust[^1] vectors:

[^1]: This post was written using Rust version 1.0.0.

{% highlight rust %}
fn main() 
{
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

There's also something similar for `Option<A>`, for `Result<A, Error>`, for pairs `(A, Second)`, and (I think this one is kind of cool) for `Box<Fn(Start) -> A>`.[^2]

[^2]: There's also instances for `Result<Value, A>` and `(First, A)`, but in my experience it's pretty rare to see them in use. When using `Fn(A) -> B` there's also instances for `(A, A)` and `Result<A, A>`, but I've encountered those even less.

Let's take a look at that one:

{% highlight rust %}
fn main() 
{
    let fun1: Box<Fn(&'static str) -> usize> = 
        Box::new(|string: &str| {string.len()});
    let fun2: Box<Fn(&'static str) -> usize> = 
        convert_fun(fun1, |len| {len * 3});
    println!("{:?}", fun2("Cool")); // prints "12"

    let fun3: Box<Fn(&'static str) -> String> = 
        convert_fun(fun2, |number| {
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

I may have lied slightly: you might have noticed the lifetime parameter `'a` in the code above being used for both the thing we're converting (`fun`) and the function we use to convert it (`f`).
This is important for making sure that the converting function `f` lives at least as long as the things it converts.
We can stick this same pattern into all the other examples, so maybe the 'right' abstraction involves functions with lifetimes.

This is a pretty simple and common pattern, but it's rare that we want to abstract over types that can implement it.
However, a lot of things that have this conversion property also have a more useful property.
Say we have a container of type `C` containing some `A`, and another container (of the same type `C`) containing a function from `A` to `B`.
We can 'internally apply' the contained function to the contained value to get a container (again, of type `C`) containing `B`
(This is different to the property above, in that the function is contained as well as the value it gets applied to).

Usually if we can do that then we can also 'wrap' values: if we have a value of type `A` we can put it 'into' the container type `C`.
As you can imagine, that part isn't as interesting to implement.
However, it's still important that we have that ability.

We can produce examples of this 'application' property for all the examples of the 'convertable' property listed above.
The examples for vectors and functions are pretty fiddly, so here's the example for `Option`:

{% highlight rust %}
fn main() 
{
    let opt_str: Option<&'static str> = wrap_opt("foo");
    let opt_fun = wrap_opt(|string: &'static str| {string.len()});
    let opt_res: Option<usize> = apply_opt(opt_str, opt_fun);
    println!("{:?}", opt_res); // prints "Some(3)"

    let very_specific_none: Option<&Fn(&'static str) -> usize> =
        None;
    let opt_other_res: Option<usize> = 
        apply_opt(opt_str, very_specific_none);
    println!("{:?}", opt_other_res); // prints "None"
}

fn apply_opt<A, B, F>
    (opt_a: Option<A>, opt_f: Option<F>) -> Option<B>
    where F: Fn(A) -> B
{
    match (opt_a, opt_f) {
        (Some(a), Some(f)) => Some(f(a)),
        _                  => None,
    }
}

fn wrap_opt<A>(a: A): Option<A>
{
    Some(a)
}
{% endhighlight %}

Now, this pattern is more useful to abstract over. In particular, say we've got something we can intuitively traverse, like a `Vec` or `Option`.
Say that our traversable container contains _another_ container `T`, which is one of these 'applicable' containers we just described.
Then we can 'transpose' the outer container and the inner 'applicable' container.
For instance, we can turn a `Vec<Option<A>>` into an `Option<Vec<A>>`, or a `Vec<Result<A, Error>>` into a `Result<Vec<A>, Error>`.
We might see that last example if, for instance, we had a vector of files and then tried to open each one.

You can probably imagine the general concept behind this pattern:
things like `Option` and `Result` encode the idea of 'normal behaviour _or something else_', so turning a `Vec<Result<A, Error>>` into a `Result<Vec<A>, Error>` probably means encoding whether there were any '_something else_' cases inside the original vector.

The type `Box<Fn(Start) -> A + 'a>` encodes the idea that 'once you give me a `Start`, I can give you an `A`' 
(It also encodes a bunch of other stuff about memory and lifetimes that isn't too relevant here).
So turning a `Vec<Box<Fn(Start) -> A + 'a>>` into a `Box<Fn(Start) -> Vec<A> + 'a>` probably encodes the idea that 'once you give me a `Start`, I can use all these functions to give you a `Vec<A>`'.

Typically we construct the result in sequence, using the original vector of applicable containers.
For instance, if we have a vector `vec![Ok(1), Err("oh no!"), Err("whoops")]`, the result would be `Err("oh no!")`.
If we had a vector of functions, the resulting function would produce a vector with elements in the same order as the original vector.
For that reason, this pattern is sometimes called 'sequencing' as opposed to 'transposing'.
We're probably going to see the `Vec<Result<A, Error>>` use-case pretty often, so let's write that up:


