use std::marker::PhantomData;

pub enum WorkResult<A> {
    Done(A),
    More(Work<A>),
}

pub struct Work<A>(Box<WorkImpl<A>>);

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

pub trait WorkImpl<A> {
    fn run(self: Box<Self>) -> WorkResult<A>;
}

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
