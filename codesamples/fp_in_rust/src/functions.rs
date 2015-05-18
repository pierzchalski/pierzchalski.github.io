pub struct Fun<'t, A, B>
{ 
    f: Box<Fn(A) -> B + 't>
}

impl<'t, A, B> 
    FnOnce<(A,)> for 
    Fun<'t, A, B> 
{
    type Output = B;

    extern "rust-call" fn call_once(
        mut self, args: (A,)) 
        -> B
    {
        (self.f)(args.0)
    }
}

impl<'t, A, B, C> 
    FnOnce<(A, B,)> for 
    Fun<'t, (A, B,), C> 
{
    type Output = C;

    extern "rust-call" fn call_once(
        mut self, args: (A, B,))
        -> C
    {
        (self.f)(args)
    }
}

impl<'t, A, B> 
    FnMut<(A,)> for 
    Fun<'t, A, B> 
{
    extern "rust-call" fn call_mut(
        &mut self, args: (A,))
        -> B
    {
        (self.f)(args.0)
    }
}

impl<'t, A, B, C> 
    FnMut<(A, B,)> for 
    Fun<'t, (A, B,), C> 
{
    extern "rust-call" fn call_mut(
        &mut self, args: (A, B,))
        -> C
    {
        (self.f)(args)
    }
}

impl<'t, A, B> Fn<(A,)> for Fun<'t, A, B> {
    extern "rust-call" fn call(
        &self, args: (A,))
        -> B
    {
        (self.f)(args.0)
    }
}


impl<'t, A, B, C> Fn<(A, B,)> for Fun<'t, (A, B,), C> {
    extern "rust-call" fn call(
        &self, args: (A, B,))
        -> C
    {
        (self.f)(args)
    }
}

pub fn curry<'t, A, B, C, F>(f: &'t F)
    -> Fun<'t, A, Fun<'t, B, C>>
    where F: Fn((A, B)) -> C,
          A: Clone + 't
{
    lift(move |a: A| { 
        lift(move |b: B| {
            f((a.clone(), b))
        })
    })
}

pub fn then<'t, A, B, C, F, G>(f: &'t F, g: &'t G)
    -> Fun<'t, A, C>
    where F: Fn(A) -> B,
          G: Fn(B) -> C
{
    lift(move |a| g(f(a)))
}

pub fn lift<'t, A, B, F>(f: F) -> Fun<'t, A, B>
    where F: Fn(A) -> B + 't
{
    Fun { f: Box::new(f) }
}

pub fn lift2<'t, A, B, C, F>(f: F)
    -> Fun<'t, (A, B), C>
    where F: Fn(A, B) -> C + 't
{
    Fun {
        f: Box::new(move |ab: (A, B)| {
            f(ab.0, ab.1)
        }) 
    }
}
