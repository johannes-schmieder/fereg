use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize,Ordering};
struct Audit;
static LIVE:AtomicUsize=AtomicUsize::new(0);
static PEAK:AtomicUsize=AtomicUsize::new(0);
fn added(n:usize) { let live=LIVE.fetch_add(n,Ordering::Relaxed)+n; PEAK.fetch_max(live,Ordering::Relaxed); }
unsafe impl GlobalAlloc for Audit {
 unsafe fn alloc(&self,l:Layout)->*mut u8 { let p=System.alloc(l);if !p.is_null(){added(l.size());}p }
 unsafe fn alloc_zeroed(&self,l:Layout)->*mut u8 { let p=System.alloc_zeroed(l);if !p.is_null(){added(l.size());}p }
 unsafe fn dealloc(&self,p:*mut u8,l:Layout) { LIVE.fetch_sub(l.size(),Ordering::Relaxed);System.dealloc(p,l); }
 unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8 { let q=System.realloc(p,l,n); if !q.is_null(){LIVE.fetch_sub(l.size(),Ordering::Relaxed);added(n);} q }
}
#[global_allocator] static ALLOCATOR:Audit=Audit;
fn main() {
 let args:Vec<String>=std::env::args().collect();let shape=&args[1];let n:usize=args[2].parse().unwrap();
 let mut a=Vec::with_capacity(n+4);let mut b=Vec::with_capacity(n+4);
 a.extend([0,0,1,1]); b.extend([0,1,0,1]);
 for i in 0..n { match shape.as_str() {
  "chain"=>{a.push((2+i/2)as u32);b.push((2+(i+1)/2)as u32);}
  "half"=>{a.push(if i<n/2 {(2+i)as u32}else{0});b.push(0);}
  "balanced"=>{a.push((i%2)as u32);b.push(((i/2)%2)as u32);}
  _=>panic!("shape")
 }}
 let levels=[1+*a.iter().max().unwrap()as usize,1+*b.iter().max().unwrap()as usize];
 let initial=vec![true;n+4];let before=LIVE.load(Ordering::Relaxed);PEAK.store(before,Ordering::Relaxed);
 let start=std::time::Instant::now();
 let result=singleton::prune_singletons(&[&a,&b],&levels,&initial,false).unwrap();
 let seconds=start.elapsed().as_secs_f64();let peak=PEAK.load(Ordering::Relaxed)-before;
 let expected=match shape.as_str(){"chain"=>n,"half"=>n/2,_=>0};
 assert_eq!(result.cascade_events,expected);assert_eq!(result.dropped_observations.len(),expected);
 for row in 0..n+4 {assert_eq!(result.active[row],row<4 || match shape.as_str(){"chain"=>false,"half"=>row>=n/2+4,_=>true});}
 let mut hash=14695981039346656037u64;for &keep in &result.active{hash=(hash^u64::from(keep)).wrapping_mul(1099511628211);}
 println!("{{\"shape\":\"{}\",\"n\":{},\"dropped\":{},\"seconds\":{:.9},\"peak_bytes\":{},\"mask_hash\":\"{:016x}\"}}",shape,n+4,expected,seconds,peak,hash);
}
