extern crate oyashio;
use oyashio::{Stream,NodeType};

fn main () {
   // let r: Vec<&u8> = sr.filter(|n| *n % 2 == 1).collect();

    let (mut st, mut sr) = Stream::new();


    st.send(0u8);
    st.send(1u8);
    st.send(2u8);
    st.send(3u8);
    st.close();

   // for n in (0..5) { sr.get(); }
    for (i,n) in sr.enumerate() {
    //    println!("{:?}",*n);
       assert_eq!(*n,i as u8);
    }
   /* let r = sr.filter(|x|*x%2==1)
        .take(2)
        .collect::<Vec<&u8>>();
    println!("{:?}",r);*/
    
}
