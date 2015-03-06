# oyashio


### example ###

```rust
extern crate oyashio;
use oyashio::{Stream};

fn main () {
    let (mut st, mut sr) = Stream::new();
	let mut sr2 = sr.clone();

    for n in (0..10) { st.send(n) }
	st.close();
	let r = sr.filter(|x|*x%2==1)
	          .take(4)
			  .collect::<Vec<&u8>>();

    println!("{:?}",r); //[1, 3, 5, 7]

    let mut vr = vec!();
	for (i,n) in sr2.enumerate() {
	    vr.push(n);
		assert_eq!(*n,i as u8);
	}
	println!("{:?}",vr); //[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
}								
```
