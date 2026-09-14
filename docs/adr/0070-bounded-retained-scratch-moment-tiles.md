# ADR 0070: Bounded retained scratch moment tiles

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-14

Retained low-memory finite moments previously issued one positional read per
observation for every Gram-column partition. The full German PPML diagnostic
reached this loop but did not finish its first finite solve before interruption.

Read a bounded tile of complete stored rows per existing partition. At stored
width S, the tile holds `S * min(N, max(1, floor(1024/S)))` doubles. Widths above
1024 retain one complete row and use multiple bounded reads. Every Gram/RHS
entry retains its observation order, selected-column mapping, weight divisor,
reference scale and fused multiply-add expressions. Scratch range, I/O and finite
validation remain authoritative. Workers read only owned native scratch.

Replace ADR 0020 paragraph 10's retained moment row-bank bound with the checked
monotone envelope `min(threads,K) * min(N*K, max(K,1024))` doubles, where K is
preflight width. A rank reduction can increase actual partition count while
stored width stays fixed: K=5 on four workers has three partitions; rank four
has four. The envelope covers that case and shrinking stored/sample dimensions.
Dense retained moments and one-shot OLS do not acquire this bank. No additional
observation-by-design matrix, numerical cache across frames, new rank policy,
weaker certificate or admission-headroom change is introduced.

The [source-bound report](../../benchmarks/reports/development/german-ppml-scratch-moments-local-2026-09-14/README.md)
records exact scalar moments, bounded reads/memory, independent oracles and paired
complete-command gains of about 29% on two German subsets. Other
scratch consumers, including residual and QR loops, remain separate work.
