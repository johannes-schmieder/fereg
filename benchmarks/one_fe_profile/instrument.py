"""Diagnostic-only timers for the SHA-pinned fixest 0.14.2 source archive.

Run on an extracted private copy. No numerical statements are replaced.
"""
import argparse
from pathlib import Path


def replace(text, old, new):
    if text.count(old) != 1:
        raise ValueError(f"expected exactly one instrumentation anchor: {old[:90]!r}")
    return text.replace(old, new)


def instrument(root):
    path = root / "R/estimation.R"
    text = path.read_text()
    start = text.index("feols = function(")
    end = text.index("\nols_fit = function(", start)
    body = text[start:end]
    body = replace(body, "  dots = list(...)", '''  .profile_last = proc.time()[["elapsed"]]
  .profile_times = list()
  .profile_mark = function(label) {
    now = proc.time()[["elapsed"]]
    .profile_times[[label]] <<- now - .profile_last
    .profile_last <<- now
  }
  dots = list(...)''')
    for anchor, label in [
        ('  isFixef = get("isFixef", env)', "environment"),
        ('    time_demean = proc.time()', "dispatch"),
        ('  # Estimation\n', "demean"),
        ('  time_post = proc.time()', "finite_fit"),
        ('    # score + hessian + vcov', "prediction"),
        ('    res$hessian = est$xwx', "scores"),
        ('  # fit stats\n', "iid_inference"),
        ('  class(res) = "fixest"', "fit_statistics"),
    ]:
        body = replace(body, anchor, f'  .profile_mark("{label}")\n' + anchor)
    body = replace(body, '    y_demean = vars_demean$y_demean\n    if(onlyFixef)',
                   '    res$profile_demean = vars_demean$profile\n    y_demean = vars_demean$y_demean\n    if(onlyFixef)')
    body = replace(body, '    coef = est$coefficients',
                   '    res$profile_ols = est$profile\n    coef = est$coefficients')
    body = replace(body, '\n  res\n}', '''
  .profile_mark("summary")
  res$profile_phases = .profile_times
  res
}''')
    text = text[:start] + body + text[end:]
    start = text.index("ols_fit = function(")
    end = text.index("\ncheck_conv = function(", start)
    body = text[start:end]
    body = replace(body, '  # No control here -- done before',
                   '  .profile_start = proc.time()[["elapsed"]]\n  # No control here -- done before')
    body = replace(body, '  multicol = FALSE',
                   '  .profile_products = proc.time()[["elapsed"]]\n  multicol = FALSE')
    body = replace(body, '  xwx_inv = info_inv$XtX_inv',
                   '  .profile_factor = proc.time()[["elapsed"]]\n  xwx_inv = info_inv$XtX_inv')
    body = replace(body, '\n  res\n}', '''
  res$profile = list(products = .profile_products - .profile_start,
                     factor = .profile_factor - .profile_products,
                     coefficients_residual = proc.time()[["elapsed"]] - .profile_factor)
  res
}''')
    path.write_text(text[:start] + body + text[end:])

    path = root / "src/demeaning.cpp"
    text = '#include <chrono>\n' + path.read_text()
    start = text.index('List cpp_demean(')
    body = text[start:]
    body = replace(body, '  // main fun that calls demean_single', '''  using ProfileClock = std::chrono::steady_clock;
  auto profile_start = ProfileClock::now();
  // main fun that calls demean_single''')
    body = replace(body, '  // output vector: (Note',
                   '  auto profile_fe = ProfileClock::now();\n  // output vector: (Note')
    body = replace(body, '  // enlever les rprintf dans les nthreads jobs',
                   '  auto profile_setup = ProfileClock::now();\n  // enlever les rprintf dans les nthreads jobs')
    body = replace(body, '  if(*(args.stopnow)){',
                   '  auto profile_projection = ProfileClock::now();\n  if(*(args.stopnow)){')
    body = replace(body, '  return(res);', '''  auto profile_finish = ProfileClock::now();
  res["profile"] = NumericVector::create(
      Named("fe_setup") = std::chrono::duration<double>(profile_fe-profile_start).count(),
      Named("workspace") = std::chrono::duration<double>(profile_setup-profile_fe).count(),
      Named("group_means") = std::chrono::duration<double>(profile_projection-profile_setup).count(),
      Named("subtract_export") = std::chrono::duration<double>(profile_finish-profile_projection).count());
  return(res);''')
    path.write_text(text[:start] + body)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    instrument(parser.parse_args().source)
