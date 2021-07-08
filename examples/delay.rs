use std::io;
use std::time::Duration;

use slings::time::delay_for;

fn main() -> io::Result<()> {
    slings::block_on(async {
        delay_for(Duration::from_secs(1)).await;
    });
    Ok(())
}
