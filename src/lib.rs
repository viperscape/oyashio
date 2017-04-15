extern crate promiser;
use promiser::{Promiser,Promisee,Promise,Latch};
use std::mem;

#[derive(Clone,Debug)]
pub enum NodeKind<T> {
    Start,
    Data(T),
    End,
}

#[derive(Clone)]
pub struct Node<T:Send+'static> {
    pub data: NodeKind<T>,
    next: Promisee<Node<T>>,
    latch: Latch,
}

//

#[derive(Clone)]
pub struct StreamR<T:Send+'static> {
    node: Promisee<Node<T>>,
    broadcast: bool,
}
impl<T:Send+'static> StreamR<T> {
    // note: this fn will likely be removed, here for now
    pub fn with<W,F:Fn(&T)->W> (&mut self, f:F) -> Option<W> {
        let mut r = None;
        let mut nn = None;
        let _ = self.node.with(|x| {
            let rv = x.next.with(|xs| {
                match xs.data {
                    NodeKind::Start => None,
                    NodeKind::End => None,
                    NodeKind::Data(ref d) => Some(f(d)),
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

        let _ = self.node.with(|x| {
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
    pub fn get (&mut self) -> Option<&NodeKind<T>> {
        let mut r = None;
        loop {
            let pr = self.shift();
            let mut lr = true;
            unsafe {
                let rv = pr.with(|x| {
                    if !self.broadcast { lr = x.latch.close(); }
                    mem::transmute(&x.data)
                });
                
                match rv {
                    Ok(v) => {
                        if lr { r = Some(v) }
                        else if !self.broadcast && !lr {
                            match v {
                                &NodeKind::End => break,
                                _=> continue,
                            }
                        }
                        else { r = None }
                    },
                    _ => r = None,
                }

                
                break;
            }
        }
        r
    }

    pub fn get_try (&mut self) -> Result<&NodeKind<T>,String> {
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

    pub fn recv (&mut self) -> Option<&'static T> {
        let mut rv = None;

        {let r = self.get();
         if r.is_some() {
             rv = match *r.unwrap() {
                 NodeKind::End => None,
                 NodeKind::Start => None, // todo: fixme! this should be skipped instead
                 NodeKind::Data(ref d) => Some(unsafe { mem::transmute(d) }),
             };
         }}
        
        // we try twice in order to skip first node
        if rv.is_some() {rv}
        else { //try once more
            let r = self.get();
            if r.is_some() {
                match *r.unwrap() {
                    NodeKind::End => None,
                    NodeKind::Start => None, // todo: fixme! this should be skipped instead
                    NodeKind::Data(ref d) => Some(unsafe { mem::transmute(d) }),
                }
            }
            else {None}
        }
    }

    pub fn recv_try (&mut self) -> Result<&'static T,String> {
        let rv;

        {let r = try!(self.get_try());
         rv = match *r {
             NodeKind::End => None,
             NodeKind::Start => None, // todo: fixme! this should be skipped instead
             NodeKind::Data(ref d) => Some(unsafe { mem::transmute(d) }),
         };
        }

        // we try twice in order to skip first node
        if rv.is_some() {Ok(rv.unwrap())}
        else { //try once more
            let r = try!(self.get_try());
            match *r {
                NodeKind::End => Err("End of Stream".to_string()),
                NodeKind::Start => Err("No next node".to_string()),
                NodeKind::Data(ref d) => Ok(unsafe { mem::transmute(d) }),
            }
        }
    }

    pub fn poll(self) -> StreamRPoll<T> {
        StreamRPoll(self)
    }
    
}
impl<T:Send+'static> Iterator for StreamR<T>  {
    type Item=&'static T;
    fn next(&mut self) -> Option<Self::Item> {
        self.recv()
    }
}
pub struct StreamRPoll<T:Send+'static>(StreamR<T>);
impl<T:Send+'static> Iterator for StreamRPoll<T>  {
    type Item=&'static T;
    fn next(&mut self) -> Option<Self::Item> {
        let r = self.0.recv_try();
        match r {
            Ok(rv) => Some(rv),
            Err(_) => None,
        }
    }
}

//
// todo: work on bounded stream by watching for strong_count on N
pub struct StreamW<T:Send+'static> {
    sink: Promiser<Node<T>>,
}
impl<T:Send+'static> StreamW<T> {
    pub fn send(&mut self, d:T) {
        let (pt,pr) = Promise::new();
        let n = Node { data: NodeKind::Data(d),
                       next: pr,
                       latch: Latch::new() };
        self.sink.deliver(n);
        self.sink = pt;
    }
    
    pub fn close(&mut self) {
        let (_,pr) = Promise::new();
        let n = Node { data: NodeKind::End,
                       next: pr,
                       latch: Latch::new() };
        self.sink.deliver(n);
    }
}


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
    pub fn default() -> (StreamW<T>,StreamR<T>) {
        let (pts,prs) = Promise::new();
        let (pt,pr) = Promise::new();
        let n = Node { data: NodeKind::Start,
                       next: pr,
                       latch: Latch::new() };
        pts.deliver(n);
        (StreamW { sink: pt },
         StreamR { node: prs,
                   broadcast: false, } )
    }

    pub fn new_broadcast() -> (StreamW<T>,StreamR<T>) {
        let (sw, mut sr) = Stream::default();
        sr.broadcast = true;
        (sw, sr)
    }
}


//


#[derive(Clone)]
pub struct StreamRMerge<T:Send+'static> {
    streams: Vec<StreamR<T>>,
    idx: usize, //current stream
    eidx: usize, //empty stream count
}

impl<T:Send+'static> StreamRMerge<T> {
    pub fn new (v:Vec<StreamR<T>>) -> StreamRMerge<T> {
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

impl<T:Send+'static> Iterator for StreamRMerge<T> {
    type Item=&'static T;
    fn next (&mut self) -> Option<Self::Item> {
        let mut r: Option<&T>;
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
    use Stream;
    use StreamRMerge;
    use std::thread;

    #[test]
    fn test_stream() {
        let (mut st,mut sr) = Stream::default();

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
        let (mut st,mut sr) = Stream::default();
        let mut sr2 = sr.clone();
        st.send(0u8);
        st.send(1u8);
        assert_eq!(sr.recv().unwrap(),&0);
        assert_eq!(sr2.recv().unwrap(),&1);
    }

    #[test]
    fn test_stream_dupe_broadcast() {
        let (mut st,mut sr) = Stream::new_broadcast();
        let mut sr2 = sr.clone();
        st.send(0u8);
        assert_eq!(sr.recv().unwrap(),&0);
        assert_eq!(sr2.recv().unwrap(),&0);
    }

    #[test]
    fn test_stream_close() {
        let (mut st,mut sr) = Stream::<u8>::default();
        st.close();
        assert_eq!(sr.recv(),None);
    }

    #[test]
    fn test_stream_drop() {
        let (mut st,mut sr) = Stream::default();
        thread::spawn(move || {
            st.send(0u8);
        });
        assert_eq!(sr.recv().unwrap(),&0);
        assert_eq!(sr.recv(),None);
    }

    #[test]
    fn test_stream_merge() {
        let (mut st2, sr2) = Stream::default();
        let (mut st, sr) = Stream::default();
        let (mut st3, sr3) = Stream::default();
        
        for n in 0..2 { st.send(n); }

        let mut vr: Vec<i32> = vec!();        
        let vsr = vec![sr,sr2,sr3];
        
        let sm = StreamRMerge::new(vsr);
        
        for n in 2..4 { st2.send(n); }
        for n in 4..6 { st3.send(n); }
        
        for n in sm.clone() { vr.push(*n); }

        assert_eq!(&vr,&[0,2,4,1,3,5]);
    }
}
