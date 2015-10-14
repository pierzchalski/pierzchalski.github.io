use func::Func;

pub trait Term {
    type Result;
    fn run(self) -> Self::Result;
}

pub struct Lift<A>(pub A);

impl<A> Term for Lift<A> {
    type Result = A;
    fn run(self) -> A {
        self.0
    }
}

pub struct Apply<F, A>(pub F, pub A);

impl<F, R, A, B> Term for Apply<F, A>
    where F: Term<Result=R>,
          R: Func<A, Output=B>
{
    type Result = B;
    fn run(self) -> B {
        (self.0.run()).call(self.1)
    }
}
