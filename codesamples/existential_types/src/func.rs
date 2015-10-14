pub trait Func<Arg> {
    type Output;
    fn call(self, arg: Arg) -> Self::Output;
}

pub struct Lift<F>(pub F);

impl<F, A, B> Func<A> for Lift<F>
    where F: FnOnce(A) -> B
{
    type Output = B;
    fn call(self, arg: A) -> B {
        (self.0)(arg)
    }
}

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
