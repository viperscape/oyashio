#![feature(unsafe_destructor)]

extern crate promise;
use promise::{Promiser,Promisee,Promise};
use std::mem;

#[derive(Clone,Debug)]
pub enum NodeType<T> {
    Start,
    Data(T),
    End,
}

#[derive(Clone)]
pub struct Node<T> {
    pub data: NodeType<T>,
    next: Promisee<Node<T>>,
}

//

#[derive(Clone)]
pub struct StreamR<T> {
    node: Promisee<Node<T>>,
}
impl<T:Send+'static> StreamR<T> {
    // note: this fn will likely be removed, here for now
    pub fn with<W,F:Fn(&T)->W> (&mut self, f:F) -> Option<W> {
        let mut r = None;
        let mut nn = None;
        self.node.with(|x| {
            let rv = x.next.with(|xs| {
                match xs.data {
                    NodeType::Start => None,
                    NodeType::End => None,
                    NodeType::Data(ref d) => Some(f(d)),
                }
            });
            
            nn = Some(x.next.clone());

            match rv {
                Ok(v) => {r=v},
                Err(_) => (),
            }
        });

        self.node = nn.unwrap();

        r
    }

    pub fn shift (&mut self) -> Promisee<Node<T>>{
        let mut nn = None;
        let ln = self.node.clone();
        self.node.with(|x| {
            nn = Some(x.next.clone()); 
        });
        if nn.is_some() {self.node = nn.unwrap();}
        ln
    }

    // todo: look in to transmute's downsides
    // eg: if data is in arc, am I ok or need manual drop?
    // should I rewrap the Result as Option, manually?
    pub fn recv (&mut self) -> Option<&NodeType<T>> {
        let pr = self.shift();
        unsafe {
            let rv = pr.with(|x| mem::transmute(&x.data));
            match rv {
                Ok(v) => Some(v),
                _ => None,
            }
        }
    }

    pub fn match_node(&mut self) -> Option<&T> {
        let mut rv = None;
        {let r = self.recv();
         if r.is_some() {
             rv = match *r.unwrap() {
                 NodeType::End => None,
                 NodeType::Start => None, // todo: fixme! this should be skipped instead
                 NodeType::Data(ref d) => Some(unsafe { mem::transmute(d) }),
             };
         }}
        if rv.is_some() {rv}
        else {None}
    }

    pub fn get (&mut self) -> Option<&T> {
        let mut rv = None;

        // note: I cannot borrow twice here, somehow below works
        // I'd like to use match_node fn twice tho
        {let r = self.recv();
         if r.is_some() {
             rv = match *r.unwrap() {
                 NodeType::End => None,
                 NodeType::Start => None, // todo: fixme! this should be skipped instead
                 NodeType::Data(ref d) => Some(unsafe { mem::transmute(d) }),
             };
         }}
        
        if rv.is_some() {rv}
        else { //try once more
            self.match_node()
        }
    }
    
}
impl<'a,T:Send+'static> Iterator for StreamR<T>  {
    type Item=&'a T;
    fn next(&mut self) -> Option<&T> {
        self.get()
    }
}

//
// todo: work on bounded stream by watching for strong_count on N
pub struct StreamW<T> {
    sink: Promiser<Node<T>>,
}
impl<T:Send+'static> StreamW<T> {
    pub fn send(&mut self, d:T) {
        let (pt,pr) = Promise::new();
        let n = Node { data: NodeType::Data(d),
                       next: pr };
        self.sink.deliver(n);
        self.sink = pt;
    }
    
    pub fn close(&mut self) {
        let (_,pr) = Promise::new();
        let n = Node { data: NodeType::End,
                       next: pr };
        self.sink.deliver(n);
    }
}
#[unsafe_destructor]
impl<T: Send+'static> Drop for StreamW<T> {
    fn drop (&mut self) {
        self.close();
    }
}

//

pub struct Stream<T> {
    _m: std::marker::PhantomData<T>,
}

impl<T:Send+'static> Stream<T> {
    pub fn new() -> (StreamW<T>,StreamR<T>) {
        let (pts,prs) = Promise::new();
        let (pt,pr) = Promise::new();
        let n = Node { data: NodeType::Start,
                       next: pr };
        pts.deliver(n);
        (StreamW { sink: pt },
         StreamR { node: prs } )
    }
}


//


#[cfg(test)]
mod tests {
    extern crate test;
    extern crate rand;
    use Stream;
    use std::thread::Thread;

    #[test]
    fn test_stream() {
        let (mut st,mut sr) = Stream::new();

        st.send(0u8);
        assert_eq!(sr.get().unwrap(),&0);

        st.send(0u8);
        st.send(1u8);
        st.send(2u8);
        st.send(3u8);
        st.close();
        for (i,n) in sr.enumerate() {
            assert_eq!(*n,i as u8);
        }
    }

    #[test]
    fn test_stream_dupe() {
        let (mut st,mut sr) = Stream::new();
        let mut sr2 = sr.clone();
        st.send(0u8);
        assert_eq!(sr.get().unwrap(),&0);
        assert_eq!(sr2.get().unwrap(),&0);
    }

    #[test]
    fn test_stream_close() {
        let (mut st,mut sr) = Stream::<u8>::new();
        st.close();
        assert_eq!(sr.get(),None);
    }

    #[test]
    fn test_stream_drop() {
        let (mut st,mut sr) = Stream::new();
        Thread::spawn(move || {
            st.send(0u8);
        });
        assert_eq!(sr.get().unwrap(),&0);
        assert_eq!(sr.get(),None);
    }

   #[bench]
    fn bench_stream(b: &mut test::Bencher) {
        let (mut st,mut sr) = Stream::new();

        b.iter(|| {
            st.send(0u8);
            sr.get().unwrap();
        });
    }

  /* #[bench]
    fn bench_stream_many(b: &mut test::Bencher) {
        let (mut st,mut sr) = Stream::new();
        let mut vsr = vec![sr.clone();1000];

        b.iter(|| {
            st.send(0u8);
            for mut n in vsr.drain() { n.recv().unwrap(); }
        });
    }*/

    #[bench]
    fn bench_stream_lots(b: &mut test::Bencher) {
        let (mut st,mut sr) = Stream::new();

        b.iter(|| {
            for n in (0..1000000) {
                st.send(n);
                sr.get().unwrap();
            }
        });
    }
}
