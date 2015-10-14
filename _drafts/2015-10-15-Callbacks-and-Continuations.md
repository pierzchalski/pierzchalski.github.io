---
layout: post
title: Callbacks and Continuations
date: 2015-10-14
summary: Almost a monad! Actually, probably a monad.
---

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

```rust
enum WorkResult<A> {
    Done(A),
    More(Box<Work<A>>),
}

trait Work<A> {
    fn run(self) -> WorkResult<A>;
}
```

This will actually get you pretty far! You can implement `map` and `flat_map` on `Work<A>` which means we can ergonomically chain large computations together:

```rust
fn long_running_initialisation() -> Work<FrameTable> {
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
    let frames = frame_loop(1024);
    frames.flat_map(|frames| {
        // TODO: do something super cool with all these frames!
    })
}
```

This is a simplified form of [thunking](https://wiki.haskell.org/Thunk), which is usually used to handle both lazy evaluation and deep recursion (by moving execution from naive recursion to controlled, stepped recursion). Unfortunately, this runs into a wall pretty quickly whenever you try and use it with callbacks. For example, consider this simple callback-based API:

```rust
extern fn callback_api<F: FnOnce(A)>(f: F);
```

If we want to use this callback with all of our other `Work` infrastructure, we clearly want to wrap `callback_api` and turn it into a `Work<A>`. But how exactly would we wrap it?

We could make the work queue public, then register a callback with `callback_api` which will append the result if needed (which won't work if the type of the callback result doesn't match the type stored in the queue). Unfortunately that doesn't solve the problem that calling `run` on a `Work` needs to synchronously return a result when called.

Hm.
_Hmmmmmm._
