---
layout: post
title: Callbacks and Continuations
date: 2015-10-14
summary: Almost a monad! Actually, probably a monad.
---

This post is using `rustc 1.5.0-nightly (11a612795 2015-10-04)`. The only unstable feature used is `fnbox`.

Over the past three months my project partner and I have been using Rust in the [Advanced Operating Systems](www.cse.unsw.edu.au/~cs9242/) (AOS) course at the [University of New South Wales, Australia](https://www.unsw.edu.au/). The course is mostly managed and run by [Data61](http://www.nicta.com.au/) (formerly NICTA). One of the lessons they get across pretty early on is the importance of making your kernel asynchronous: it's going to be doing a lot of long-running things like file IO (which will invariably provide a callback-based API), but at the same time it needs to be responsive to things like syscalls and user virtual memory faults.

One of the reasons this gets discussed early on is because your choice of execution model to deal with asynchronous behavior heavily affects the design of your kernel, and is usually pretty hard to migrate afterwards because whatever model you choose becomes pervasive through your kernel. The lecturers presented us with a few rough categories of execution models:

* **Threads:** these are the standard way of dealing with asynchronous things. You fork a thread to do some long-running computation, using some kind of synchronization primitive to block until it's done. For AOS, the coursework involves building your kernel on top of the microkernel [seL4](https://sel4.systems/), which provides most of the tools you need to actually manage creation, scheduling, and execution of threadlike things, but you still need to manage your own stacks and synchronization primitives.

* **Coroutines:** while threads change between execution contexts whenever some external scheduler decides to make it happen, coroutines require explicit calls to control-managing functions. The idea is that the current function runs for a bit, then calls some function which returns control to the caller. The caller then has the ability to 'resume' the continuation whenever is convenient. This also involves managing stacks, and sometimes hides where asynchronous behavior is happening.

* **Events:** this is the model used internally by seL4 because of its simplicity. Assume you have a queue of events. Make a loop somewhere which can fetch the next element of the queue, or wait until something gets added to it. Dealing with callback-based API's is pretty straightforward: make the callback append an event to the queue, then deal with the event later. You can even split up long-running operations by turning them into a sequence of events that each get added to the queue.

* **Callbacks/Continuations:** this is the functional programming solution. The idea is to program everything through a type equivalent to `(A -> R) -> R` (among [many ways](https://wiki.haskell.org/Continuation) to think about them, one is to say that the passed-in continuation represents 'the rest' of the computation, and so manipulating how it gets called in turn manipulates control flow).

Although our kernel went through a few versions where we had a weird mix of threads and work queues, we finally settled on a design that we think works quite well and plays to Rust's strengths, and which is a neat little mix of events and continuations.

### Work Chunks and Queue Thunks

In hindsight, there were two overarching factors that most strongly influenced our design:

1. The NFS library and serial port library we were provided with had a callback-based API, and
2. There would be times when the kernel would be doing either a large number of small operations or a small number of large operations, and we would need to be able to partition that work and interleave other things like users making syscalls or VM faults.

The need to chunk and pause work hinted at having a work queue, pretty much a `LinkedList<Work<A>>`. We could pop off a chunk of work, a `Work<A>`, then run it and get either an `A` to return or another `Work<A>` to push onto the queue. That sounds like it implies a pretty obvious and particular definition:

<a name="definition-work"></a>

```rust
enum WorkResult<A> {
    Done(A),
    More(Work<A>),
}

struct Work<A>(Box<WorkImpl<A>>);

trait WorkImpl<A> {
    fn run(self: Box<Self>) -> WorkResult<A>;
}

struct WorkQueue<A>(LinkedList<Work<A>>);

impl<A> WorkQueue<A> {
    fn step(&mut self) -> Option<A> {
        if let Some(work) = self.0.pop_front() {
            match work.run() {
                WorkResult::Done(a) =>
                  return Some(a),
                WorkResult::More(work) =>
                  self.0.push_back(work),
            }
        }
        None
    }
}
```

This will actually get you pretty far! You can implement `map` and `flat_map` on `Work<A>` (I've put a simple implementation at the [bottom](#appendix-work) of this post, which also explains why we have the indirection using `Box`, as well as the separation between `Work` and `WorkImpl`). That lets us ergonomically chain large computations together:

```rust
fn long_running_initialisation(num_frames: usize)
    -> Work<FrameTable>
{
    fn frame_loop(to_get: usize) -> Work<Vec<Frame>> {
        if to_get == 0 { return Work::Done(Vec::new()) }
        let frame: Work<Frame> = long_running_frame_getting_thing();
        frame.flat_map(|frame| {
            frame_loop(to_get - 1).map(move |mut frames| {
                frames.push(frame);
                frames
            })
        })
    }
    // returns instantly, will do the work later
    let frames = frame_loop(num_frames);
    frames.flat_map(|frames| {
        // TODO: do something super cool with all these frames!
    })
}
```

Push that onto a `WorkQueue` then call `WorkQueue::step` until you get what you want, or stop early if you need to! This is a simplified form of [thunking](https://wiki.haskell.org/Thunk), which is typically used to handle both lazy evaluation and deep recursion (by moving execution from naive recursion on the stack to controlled, stepped recursion through data). Unfortunately, this runs into a wall pretty quickly whenever you try and use it with callbacks. For example, consider this simple callback-based API:

```rust
extern fn callback_api<F: FnOnce(A)>(f: F);
```

If we want to use this callback with all of our other `Work` infrastructure, we probably want to wrap `callback_api` and turn it into a `Work<A>`. But how exactly would we wrap it?

We could make the work queue public, then register a callback with `callback_api` which will append the result if needed (which won't work anyway if the type of the callback result doesn't match the type stored in the queue). Unfortunately, that's both a pretty bad code smell (we're sprinkling code that cares about handling work into code that's meant to just be generating work), and it also doesn't solve the problem that calling `Work::run` needs to synchronously return a result when called.

Hm.
_Hmmmmmm._
What if it _didn't_ need to?

### Give Me Your Future

This is where we steal the key idea behind continuations and then twist it into our own weird version. Remember the type of continuations (`(A -> R) -> R`) and the interpretation that the function argument of type `A -> R` represents 'the rest of the program'? We're gonna use that.

Thankfully, despite that weird `R` floating around in the type signature above, in our case our work is always purely side-effecting: at the top level, our work queue doesn't store or return anything, it just pops work and runs it. So for our case, we're going to pretend that continuations just look like `(A -> ()) -> ()`.

We're gonna assume that each unit of work knows what it needs to do with the 'rest' of the program, so our new definition looks something like this (with the name changed to `Cont` to help distinguish them):

```rust
enum ContResult<A> {
    Done(A),
    More(Cont<A>),
}

struct Cont<A>(Box<ContImpl<A>>);

struct End<A>(Box<FnBox(ContResult<A>)>);

trait ContImpl<A> {
    fn run(self: Box<Self>, end: End<A>);
}
```

<a name="handling-callbacks"></a>
Again, we can implement `map` and `flat_map` for `Cont<A>`, so we can pretty much copy the toy example code from above (and there _is_ a bit of a trick to it, outlined [here](#appendix-cont)), but the real gain is that this is enough for us to wrap `callback_api` from above:

```rust

extern fn callback_api<F: FnOnce(A)>(f: F);

struct Callback;

impl ContResult<A> for Callback {
    fn run(self: Box<Self>, end: End<A>) {
        callback_api(|a| {
            end.run(ContResult::Done(a));
        })
    }
}
```

And that's it. Add a bunch of utility methods (maybe something that lets you lift closures that take in an `End<A>` so you don't have to make dummy structs like `Callback` above) and you've got a perfectly decent callback-wrapping, continuation-based library. I might write a blog post about how we ended up going macro crazy wrapping an NFS library with these continuations.

The one thing missing now is how to actually process these units of work. Our queue from before won't do as-is, since now our continuations don't actually return anything when they run them. We also have to figure out what to pass in as the initial value for our `End<A>` continuation parameter. The solution is pretty straightforward: pass in a function that will enqueue the result onto the queue!

```rust
struct ContQueue(Rc<RefCell<LinkedList<Cont<()>>>>);

impl ContQueue {
    fn step(&mut self) {
        let cont = {
            // make sure the mutable borrow is as
            // short as possible using braces
            self.0.borrow_mut().pop_front()
        };
        match cont {
            Some(cont) => {
                // copy the Rc<RefCell<_>>
                let cq = self.0.clone();
                let fun = move |result| {
                    match result {
                        ContResult::Done(_) => { }
                        ContResult::More(c) => {
                            cq.borrow_mut()
                                .push_back(c);
                        }
                    }
                }
                let end = End(Box::new(fun));
                cont.run(end);
            }
            None => { }
        }
    }
}
```

Now all you've got to do is figure out when to actually step in the queue. Probably once you get an interrupt or event from the ever-helpful seL4 or something. Have fun!

### <a name="appendix-work"></a> Appendix: Map and Flat Map for `Work<A>`

The definition for `Work` that we defined [earlier](#definition-work) might look a little worrying with the layers of indirection, but the rationale is pretty straightforward. To implement `map` and `flat_map`, we need to create a structure that carries around the work we've done so far, as well as the closure to apply once we're ready. If we try and implement that naively, we run into a pair of problems:

```rust
enum WorkResult<A> {
    Done(A),
    More(Work<A>),
}

enum Work<A> {
    Map<F, A> {
        work: Work<A>, // directly recursive type!
        fun: F, // undefined type F!
    },
    FlatMap<F, A> {
        work: Work<A>,
        fun: F,
    },
    // probably a few other variants as well
}
```

The first issue is that we end up with data definition recursion, and so the compiler can't determine a finite size for the `Work` enum when storing it (if a `Work<A>` is, say, 10 words long, how big is a `Work::Map<F, A>`?).

The second is that the insides of the enum variants 'leak' to the outer definition: we track some hidden closure of type `F` inside each variant, but since Rust needs to know how to represent a `Work<A>` it will need to know how to represent an `F`, and so will anyone calling a function that returns a `Work<A>` (this is a simple form of existential type: we're trying to express that if you have a `Work::Map<A>`, there _exists_ some closure type `F` contained in that data which you can use).

The only real solution to the first problem is to have a layer of reference indirection, either using simple references or some kind of smart pointer. Since we're implementing work thunking to put on a work queue, we need all our data to be storable and we're only ever going to use it once, so let's jump right in and make our indirection use a heap-allocated `Box`:

```rust
enum Work<A> {
    Map<F, A> {
        work: Box<Work<A>>,
        fun: F, // undefined type F!
    },
    FlatMap<F, A> {
        work: Box<Work<A>>,
        fun: F,
    },
    // probably a few other variants as well
}
```

To solve the second problem we can use dynamic dispatch, which Rust provides through a feature called [trait objects](https://doc.rust-lang.org/book/trait-objects.html). What we want is a way to implement some methods for a struct like `Map` or `FlatMap`, then wrap it in something that hides all the gory details (like what closure type it's enclosing) which we can then pass on to something expecting a `Work`. In our case that means replacing matching on the `Work` enum with calling functions on some trait that implements whatever `Work` needs (call it `WorkImpl`). Once we do that we're pretty close to the solution outlined above:

```rust
enum WorkResult<A> {
    Done(A),
    More(Work<A>),
}

// if something wants to be usable
// as a Work<A>, it just needs to implement
// WorkImpl<A>
struct Work<A>(Box<WorkImpl<A>>);

trait WorkImpl<A> {
    // we make the trait method take in a Box<Self>
    // so that we can call WorkImpl::run on
    // things that we pull out of a Work<A>.
    // The magic type 'Self' here is the type
    // of the struct that's implementing the trait,
    // so we can access anything stored in that struct.
    fn run(self: Box<Self>) -> WorkResult<A>;
}
```

Now all we need to do is implement `WorkImpl` for our `Map` and `FlatMap` structs:

```rust
struct Map<F, A> {
    work: Work<A>,
    fun: F,
}

impl<F, A, B> WorkImpl<B> for Map<F, A>
    where
        F: 'static + FnOnce(A) -> B,
        A: 'static,
{
    fn run(self: Box<Self>) -> WorkResult<B> {
        let s = *self;
        let fun = s.fun;
        let work = s.work;
        match work.run() {
            WorkResult::Done(a) =>
                WorkResult::Done(fun(a)),
            WorkResult::More(wa) =>
                WorkResult::More(Work::lift(Map {
                    work: wa,
                    fun: fun,
                })),
        }
    }
}

struct FlatMap<F, A> {
    work: Work<A>,
    fun: F,
}

impl<F, A, B> WorkImpl<B> for FlatMap<F, A>
    where
        F: 'static + FnOnce(A) -> Work<B>,
        A: 'static,
{
    fn run(self: Box<Self>) -> WorkResult<B> {
        let s = *self;
        let fun = s.fun;
        let work = s.work;
        match work.run() {
            WorkResult::Done(a) =>
                WorkResult::More(fun(a)),
            WorkResult::More(wa) =>
                WorkResult::More(Work::lift(FlatMap {
                    work: wa,
                    fun: fun,
                }))
        }
    }
}
```

And finally, add some utility functions to our `Work` struct to more easily wrap ourselves in these `Map` and `FlatMap` structs:

```rust
impl<A> Work<A> {
    pub fn map<F, B>(self, fun: F) -> Work<B>
        where
            A: 'static,
            F: 'static + FnOnce(A) -> B
    {
        Work(Box::new(Map {
            work: self,
            fun: fun,
        }))
    }

    pub fn flat_map<F, B>(self, fun: F) -> Work<B>
        where
            A: 'static,
            F: 'static + FnOnce(A) -> Work<B>
    {
        Work(Box::new(FlatMap {
            work: self,
            fun: fun,
        }))
    }

    fn run(self) -> WorkResult<A> {
        self.0.run()
    }

    fn lift<W: 'static + WorkImpl<A>>(work_impl: W)
        -> Self
    {
        Work(Box::new(work_impl))
    }
}
```

And we're now at the point where the little code sample from earlier will compile (Once we have some toy definitions for the functions)!

As an aside, the reason for all those `'static`s in the above code is that every struct we've defined so far needs to be storable for arbitrarily long periods of time (remember, all these chunks of work get shoved on a queue). That means they can't contain anything that might stop being valid after some point of execution, like references or smart borrow structs like a [`RefCell`](https://doc.rust-lang.org/std/cell/struct.RefCell.html)'s [`Ref`](https://doc.rust-lang.org/std/cell/struct.Ref.html). The `'static` lifetime bound forces that requirement (and, just to be clear, the Rust compiler told me to put that there! I didn't need to realize that was necessary for safety, which is good because I _didn't_).

#### Tradeoffs

This design doesn't come for free: using `Box` invokes the overhead of `malloc` and `free` for every step of evaluation (although you might be able to avoid that with some `unsafe` code to only reallocate if it's necessary), and the dynamic dispatch adds a pointer indirection which probably leads to bad instruction cache locality (although that might be mitigated by whatever lookahead magic is on CPUs these days).

One alternative is to try and more closely copy Rust's [`Iterator`](https://doc.rust-lang.org/std/iter/trait.Iterator.html) idiom, where instead of making work objects that destroy themselves returning new work objects, we create one work object that we can mutate repeatedly until the work is done, and provide means to chain such mutable objects together. That involves its own tradeoffs (type errors become inscrutable very quickly, ownership and resource management is more opaque, and recursive definitions become difficult), but most importantly it doesn't interact well with callbacks, which is why we don't explore it much here.

### <a name="appendix-cont"></a> Appendix: Map and Flat Map for `Cont<A>`

When using continuations and callbacks, there are two things we want to do:

* Delay running the rest of the program until something says we can continue, or
* Change what happens when the rest of the program eventually runs.

We describe the first case when we talk about handling callbacks [above](#handling-callbacks), so we focus on the second case here. The trick is to build the continuation incrementally, before passing it on. There's not much more to describe, so here's the code:

```rust
struct Map<F, A> {
    cont: Cont<A>,
    fun: F,
}

impl<F, A, B> ContImpl<B> for Map<F, A>
    where
        A: 'static,
        B: 'static,
        F: 'static + FnOnce(A) -> B,
{
    fn run(self: Box<Self>, end: End<B>) {
        let s = *self;
        let cont = s.cont;
        let fun = s.fun;
        // jump in and replace 'end' with
        // 'run fun then end'
        cont.run(End(Box::new(move |result| {
            match result {
                ContResult::Done(a) => {
                    let b = fun(a);
                    let cb = ContResult::Done(b);
                    end.run(cb);
                }
                ContResult::More(ca) => {
                    let cb = ContResult::More(Cont::lift(
                        Map {
                            cont: ca,
                            fun: fun,
                        }
                    ));
                    end.run(cb);
                }
            }
        })))
    }
}

struct FlatMap<F, A> {
    cont: Cont<A>,
    fun: F,
}

impl<F, A, B> ContImpl<B> for FlatMap<F, A>
    where
        A: 'static,
        B: 'static,
        F: 'static + FnOnce(A) -> Cont<B>,
{
    fn run(self: Box<Self>, end: End<B>) {
        let s = *self;
        let cont = s.cont;
        let fun = s.fun;
        // jump in and replace 'end' with
        // 'run fun then end'
        cont.run(End(Box::new(move |result| {
            match result {
                ContResult::Done(a) => {
                    let cb = ContResult::More(fun(a));
                    end.run(cb);
                }
                ContResult::More(ca) => {
                    let cb = ContResult::More(Cont::lift(
                        FlatMap {
                            cont: ca,
                            fun: fun,
                        }
                    ));
                    end.run(cb);
                }
            }
        })))
    }
}
```
