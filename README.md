# oyashio

A single producer, multiple consumer stream
- consumer streams can be cloned
- are consumed independently
- data sits in central arc
- polling and await style consumption
- plugs in to regular iterator functions

### example ###

```rust
extern crate oyashio;
use oyashio::{Stream};

fn main () {
    let (mut st, mut sr) = Stream::new();

    for n in (0..10) { st.send(n) }

    // regular iterator, may wait for values to be sent
    let r = sr.clone().filter(|x|*x%2==1)
              .take(4) //only wait on up to 4 odd numbers
              .collect::<Vec<&u8>>();
    println!("{:?}",r); //[1, 3, 5, 7]

    
    let mut vr = vec!();
    for (i,n) in sr.clone().enumerate() { //iterates til break
        vr.push(n);
        assert_eq!(*n,i as u8);
        if i == 9 {break;} //let's break out of this
    }
    println!("{:?}",vr); //[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]

    vr = vec!();
    // polling iterator, never waits
    for n in sr.poll() { vr.push(n); }
    println!("{:?}",vr); //[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    
    
    //stream merge example
    let (mut st2,mut sr2) = Stream::new();
    let (mut st, mut sr) = Stream::new();
    let (mut st3, mut sr3) = Stream::new();
    
    for n in (0i32..2) { st.send(n); } //0,1

    let mut vr: Vec<i32> = vec!();        
    let mut vsr = vec![sr,sr2,sr3];
    
    let sm = StreamRMerge::new(vsr);
    
    for n in (2..4) { st2.send(n); } //2,3
    for n in (4..6) { st3.send(n); } //4,5
    
    for n in sm.clone() { vr.push(*n); }

    assert_eq!(vr.as_slice(),&[0,2,4,1,3,5]);
}								
```

## todo ##
- create optional bounded size streams
- let streams consumption start and stop without 'moving' stream variable (eg: stream->filter->take->collect, then stream->enumerate() without necessary cloning)
- try in real world scenarios
