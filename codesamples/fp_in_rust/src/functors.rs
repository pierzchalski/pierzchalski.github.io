use functions::*;

pub fn fmap_fun<'t, Start, A, B, F>(
    fa: &'t Fun<'t, Start, A>, f: &'t F)
    -> Fun<'t, Start, B>
    where F: Fn(A) -> B
{
    then(fa, f)
}
