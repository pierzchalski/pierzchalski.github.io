#![feature(unboxed_closures, core)]

mod functions;
mod functors;

use functions::*;

fn main() 
{   
    then_demo();
}

fn then_demo()
{
    let len = 
        |s: String| {s.len()};
    let double = 
        *Box::new(|size: usize| {2 * size});
    let pretty = 
        lift(|size: usize| {format!("Got num {}", size)});

    let s: String = "Hooray!".to_string();

    let pretty_len = then(&len, &pretty);
    let double_len = then(&len, &double);
    let all = then(&double_len, &pretty);
    println!("{}", pretty_len(s.clone()));
    println!("{}", all(s.clone()));
}
