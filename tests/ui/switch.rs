fn switch(x: i32) -> i32 {
    match x {
        0 => 99,
        2 => 99 - 2,
        x => 100 - x,
    }
}
