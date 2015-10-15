use std::boxed::FnBox;

pub enum ContResult<A> {
    Done(A),
    More(Cont<A>),
}

pub struct Cont<A>(Box<ContImpl<A>>);

impl<A> Cont<A> {
    pub fn lift<C: 'static + ContImpl<A>>(cont: C)
        -> Self
    {
        Cont(Box::new(cont))
    }

    pub fn run(self, end: End<A>) {
        self.0.run(end);
    }
}

pub struct End<A>(Box<FnBox(ContResult<A>)>);

impl<A> End<A> {
    fn run(self, result: ContResult<A>) {
        self.0(result)
    }
}

pub trait ContImpl<A> {
    fn run(self: Box<Self>, end: End<A>);
}

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
