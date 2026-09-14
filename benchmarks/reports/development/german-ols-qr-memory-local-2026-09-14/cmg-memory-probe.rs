use cmg::{CmgMemoryEstimate, CmgProblemSize, CmgOptions, ParallelOptions};
fn main() {
 let n=88_876_264_usize; let levels=14_958_347_usize;
 for width in [1_usize,10] {
  let m=CmgMemoryEstimate::conservative(CmgProblemSize{vertices:levels,input_edges:n,canonical_edges:n,right_hand_sides:width},CmgOptions::default(),ParallelOptions{threads:8,..Default::default()}).unwrap();
  println!("rhs={width} build_peak={} workspace_each={} workspace_pool={} extra_graph_vectors={}",m.build_peak_bytes(),m.workspace_bytes_each(),m.workspace_pool_bytes(),levels*width*16);
 }
}
