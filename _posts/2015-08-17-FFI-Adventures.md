---
layout: post
title: FFI Adventures in Rust
date: 2015-08-17
summary: Or, why everything is terrible forever.
---

A quick PSA to aspiring low-level Rust writers: the C ABI is dark and full of terrors. Today a friend and I made two uninformed assumptions about Rust's representation of types, and a world of sorrow followed. Thankfully, our errors were pretty minor and easy to deal with.

### Lesson 1: Learn Your Rust representations

Rust has two neat little types for dealing with arrays: `[A]` and `&[A]`. 
The former represents the actual data: if I have an instance of `[A]` then I have some unknown number of instances of `A` in a row 
(if I knew how many I had at compile time,then I would just have an instance of `[A; n]` for some known number `n`). 
The latter represents a _view_ of the data: if I have an `&[A]`, then I have a pointer that points to some `A`s in a row, along with a number which tells me how many `A`s there are
(again, if I knew how many there were at compile time, I would just have `&[A; n]` for some `n`).

Now, if you're silly like me, when you see `&[A]` you think 'that must be a pointer to a thing! Clearly I can return it from a Rust function, and just treat it like a pointer-returning C function!'.
In particular, maybe you think that if you type this in Rust:

{% highlight rust %}
extern "C" fn foo(x: Bar) -> &'static [Baz]
{
//things
}
{% endhighlight %}

Then you can type this in C:

{% highlight c %}

extern slice_t *foo(bar_t x);

{% endhighlight %}

Haha no.

Maybe you can see where I'm going with this: if an `&[A]` has a pointer _and a size_, then we're actually returning a struct, something like

{% highlight rust %}
#[repr(C)]
struct Slice<A> {
  data: *mut A, 
  size: usize,
}
{% endhighlight %}

So clearly our C signature is terribly wrong: we're going to return a struct, then interpret it as a pointer!

_Hahaha no_. It's worse than that.

### Lesson 2: Learn Your C ABI Optimizations

We noticed something weird was happening when we saw something like the following:

In C:

{% highlight c %}

bar_t x = bar_maker(5);
printf("%d\n", x.magic); // prints 0xdeadbeef
foo(x);

{% endhighlight %}

In Rust:

{% highlight rust %}
extern "C" fn foo(x: Bar) -> &'static [Baz]
{
  println!("{}", x.magic); //prints 0xf00d1234, what!?
  //things
}
{% endhighlight %}

What was going on?

When you were learning C, you probably noticed that no-one actually _returns_ things, they just mutate things through some pointers they get given as arguments.
No-one just returns structs directly from functions!
But _if you do_, and you're on the right architecture, and if you've been very good, the C ABI has a nice 
[little](https://en.wikipedia.org/wiki/Return_value_optimization) 
[present](http://stackoverflow.com/questions/8728790/why-doesnt-c-code-return-a-struct/8728932#8728932) for you: it will silently add a 
pointer parameter to the start of your parameter list, and expect your function to write its' output to that pointer.

So, this is what we told C our Rust function was:

{% highlight c %}

extern slice_t *foo(bar_t x);

{% endhighlight %}

This is what Rust thought C thought our function was:

{% highlight c %}

void foo(slice_t *slice_ret, bar_t x);

{% endhighlight %}

So all those times we called `foo(x)` from C, Rust would find some random stuff on the stack after `x`, think that that was our `Thing bar`, and then write complete garbage to wherever `x` 'points to'... and unsurprisingly fault!


