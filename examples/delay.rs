use std::io;
use std::time::Duration;

use slings::runtime::Runtime;
use slings::time::delay_for;

fn main() -> io::Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        delay_for(Duration::from_secs(1)).await;
    });
    Ok(())
}
