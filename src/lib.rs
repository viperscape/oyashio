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

    // todo: consider replacing node with Option<node>
    // this fn always is ok otherwise
    pub fn shift_try (&mut self) -> Result<Promisee<Node<T>>,String> {
        let mut nn = None;
        let ln = self.node.clone();

        {let r = try!(self.node.get());
         if r.is_some() { nn = Some(r.unwrap().next.clone()); }
        }

        if nn.is_some() {self.node = nn.unwrap();}
        Ok(ln)
    }

    // todo: look in to transmute's downsides
    // should I rewrap the Result as Option, manually?
    pub fn get (&mut self) -> Option<&NodeType<T>> {
        let pr = self.shift();
        unsafe {
            let rv = pr.with(|x| mem::transmute(&x.data));
            match rv {
                Ok(v) => Some(v),
                _ => None,
            }
        }
    }

    pub fn get_try (&mut self) -> Result<&NodeType<T>,String> {
        let pr = try!(self.shift_try());
        let r = try!(pr.get());
        match r {
            None => Err("no current value".to_string()),
            Some(v) => unsafe {
                Ok(mem::transmute(&v.data))
            },
        }
    }

   /* pub fn match_node(&self, r: &NodeType<T>) -> Option<&T> {
        let mut rv = None;
        {rv = match *r {
            NodeType::End => None,
            NodeType::Start => None, // todo: fixme! this should be skipped instead
            NodeType::Data(ref d) => Some(unsafe { mem::transmute(d) }),
        };
        }
        if rv.is_some() {rv}
        else {None}
    }*/

    pub fn recv (&mut self) -> Option<&T> {
        let mut rv = None;

        {let r = self.get();
         if r.is_some() {
             rv = match *r.unwrap() {
                 NodeType::End => None,
                 NodeType::Start => None, // todo: fixme! this should be skipped instead
                 NodeType::Data(ref d) => Some(unsafe { mem::transmute(d) }),
             };
         }}
        
        // we try twice in order to skip first node
        if rv.is_some() {rv}
        else { //try once more
            let r = self.get();
            if r.is_some() {
                match *r.unwrap() {
                    NodeType::End => None,
                    NodeType::Start => None, // todo: fixme! this should be skipped instead
                    NodeType::Data(ref d) => Some(unsafe { mem::transmute(d) }),
                }
            }
            else {None}
        }
    }

    pub fn recv_try (&mut self) -> Result<&T,String> {
        let mut rv = None;

        {let r = try!(self.get_try());
         rv = match *r {
             NodeType::End => None,
             NodeType::Start => None, // todo: fixme! this should be skipped instead
             NodeType::Data(ref d) => Some(unsafe { mem::transmute(d) }),
         };
        }

        // we try twice in order to skip first node
        if rv.is_some() {Ok(rv.unwrap())}
        else { //try once more
            let r = try!(self.get_try());
            match *r {
                NodeType::End => Err("End of Stream".to_string()),
                NodeType::Start => Err("No next node".to_string()),
                NodeType::Data(ref d) => Ok(unsafe { mem::transmute(d) }),
            }
        }
    }

    pub fn poll(self) -> StreamRPoll<T> {
        StreamRPoll(self)
    }
    
}
impl<'a,T:Send+'static> Iterator for StreamR<T>  {
    type Item=&'a T;
    fn next(&mut self) -> Option<&T> {
        self.recv()
    }
}
pub struct StreamRPoll<T>(StreamR<T>);
impl<'a,T:Send+'static> Iterator for StreamRPoll<T>  {
    type Item=&'a T;
    fn next(&mut self) -> Option<&T> {
        let r = self.0.recv_try();
        match r {
            Ok(rv) => Some(rv),
            Err(_) => None,
        }
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


#[derive(Clone)]
struct StreamRMerge<T> {
    streams: Vec<StreamR<T>>,
    idx: usize, //current stream
    eidx: usize, //empty stream count
}

impl<T:Send+'static> StreamRMerge<T> {
    fn new (v:Vec<StreamR<T>>) -> StreamRMerge<T> {
        StreamRMerge { streams:v,idx:0,eidx:0 }
    }
    fn alt (&mut self) -> Option<&T>  {
        if self.idx == self.streams.len() { self.idx = 0; }
        let r = &mut self.streams[self.idx];
        self.idx += 1;
        let r = r.recv_try();
        match r {
            Ok(v) => {
                Some(v)
            },
            Err(_) => {
                self.eidx += 1;
                None
            }
        }
    }
}

impl<'a,T:Send+'static> Iterator for StreamRMerge<T> {
    type Item=&'a T;
    fn next (&mut self) -> Option<&T> {
        let mut r = None;
        loop { 
            unsafe {r = mem::transmute(self.alt());}
            if r.is_some() { 
                break;
            }
            else { 
                if self.eidx == self.streams.len() { break; }
            }
        }

        self.eidx = 0;

        r
    }
}

//

#[cfg(test)]
mod tests {
    extern crate test;
    extern crate rand;
    use Stream;
    use StreamRMerge;
    use std::thread::Thread;

    #[test]
    fn test_stream() {
        let (mut st,mut sr) = Stream::new();

        st.send(0u8);
        assert_eq!(sr.recv().unwrap(),&0);

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
        assert_eq!(sr.recv().unwrap(),&0);
        assert_eq!(sr2.recv().unwrap(),&0);
    }

    #[test]
    fn test_stream_close() {
        let (mut st,mut sr) = Stream::<u8>::new();
        st.close();
        assert_eq!(sr.recv(),None);
    }

    #[test]
    fn test_stream_drop() {
        let (mut st,mut sr) = Stream::new();
        Thread::spawn(move || {
            st.send(0u8);
        });
        assert_eq!(sr.recv().unwrap(),&0);
        assert_eq!(sr.recv(),None);
    }

    #[test]
    fn test_stream_merge() {
        let (mut st2,mut sr2) = Stream::new();
        let (mut st, mut sr) = Stream::new();
        let (mut st3, mut sr3) = Stream::new();
        
        for n in (0i32..2) { st.send(n); }

        let mut vr: Vec<i32> = vec!();        
        let mut vsr = vec![sr,sr2,sr3];
        
        let sm = StreamRMerge::new(vsr);
        
        for n in (2..4) { st2.send(n); }
        for n in (4..6) { st3.send(n); }
        
        for n in sm.clone() { vr.push(*n); }

        assert_eq!(vr.as_slice(),&[0,2,4,1,3,5]);
    }

   #[bench]
    fn bench_stream(b: &mut test::Bencher) {
        let (mut st,mut sr) = Stream::new();

        b.iter(|| {
            st.send(0u8);
            sr.recv().unwrap();
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
}
