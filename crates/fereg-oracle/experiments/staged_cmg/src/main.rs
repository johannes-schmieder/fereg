mod local;
mod preconditioner;
mod problem;
mod solve;
#[cfg(test)]
mod tests;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        (5..=6).contains(&args.len()),
        "usage: staged-cmg-prototype CASE FIRMS RHS_COUNT ARM [cut|random]"
    );
    let p = problem::generate(&args[1], args[2].parse().unwrap());
    let count: usize = args[3].parse().unwrap();
    let pattern = args.get(5).map_or("cut", String::as_str);
    assert!(["cut", "random"].contains(&pattern));
    let rhs = if pattern == "random" {
        p.rhs(count + 1).into_iter().skip(1).collect()
    } else {
        p.rhs(count)
    };
    match solve::run(&p, &rhs, &args[4]) {
        Ok(r) => println!(
            concat!(
                "{{\"status\":\"ok\",\"case\":\"{}\",\"n\":{},\"rhs\":{},\"arm\":\"{}\",",
                "\"seconds\":{:.9},\"setup_seconds\":{:.9},\"graph_seconds\":{:.9},",
                "\"apply_seconds\":{:.9},\"iterations\":{},\"upgrades\":{},\"certificates\":{},",
                "\"relative_residual\":{:.17e},\"digest\":\"{:016x}\",\"graph_builds\":{},",
                "\"coarse_n\":{},\"tail_levels\":{},\"decisions\":[{}]}}"
            ),
            args[1],
            p.n,
            rhs.len(),
            args[4],
            r.seconds,
            r.setup,
            r.graph_seconds,
            r.apply_seconds,
            r.iterations,
            r.upgrades,
            r.certificates,
            r.residual,
            r.digest,
            r.graph_builds,
            r.coarse_n,
            r.tail_levels,
            r.decisions.join(",")
        ),
        Err(e) => {
            println!("{{\"status\":\"failed\",\"reason\":\"{e}\"}}");
            std::process::exit(2);
        }
    }
}
