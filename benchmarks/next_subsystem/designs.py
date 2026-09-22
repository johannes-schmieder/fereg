"""Additive topology controls; never modifies the frozen synthetic generator."""
import datetime
import hashlib
import json

import numpy as np
import pandas as pd


def augment(cell, path):
    topology = cell.get("topology")
    if not topology:
        return
    frame = pd.read_stata(path, convert_categoricals=False)
    i = np.arange(len(frame), dtype=np.int64)
    if topology == "disconnected":
        # Whole five-row workers remain intact; split each other FE label by
        # worker component, giving four disjoint subgraphs without deletion.
        component = (frame.g1.to_numpy(dtype=np.int64) - 1) % 4
        for name in cell["effects"]:
            frame[name] = frame[name] + component * int(frame[name].max())
    elif topology == "unbalanced":
        # Unequal group sizes from a deterministic many-to-one worker map.
        frame.g1 = np.floor(np.sqrt(frame.g1.to_numpy()) * 4).astype(np.int64) + 1
    elif topology == "four_fe":
        frame["g4"] = (i // 5 + 13 * (i % 5)) % 23 + 1
    else:
        raise ValueError(f"Unknown topology control: {topology}")
    frame.to_stata(path, write_index=False, version=118, time_stamp=datetime.datetime(2000, 1, 1))
    assert np.array_equal(frame.to_numpy(), pd.read_stata(path, convert_categoricals=False).to_numpy())
    info = json.loads(path.with_suffix(".json").read_text())
    info.update(topology=topology, sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        levels={name: int(frame[name].nunique()) for name in cell["effects"]},
        minimum_level_rows={name: int(frame.groupby(name).size().min()) for name in cell["effects"]},
        columns=list(frame.columns),
        column_sha256={name: hashlib.sha256(np.asarray(frame[name], dtype="<f8").tobytes()).hexdigest() for name in frame})
    path.with_suffix(".json").write_text(json.dumps(info, indent=2) + "\n")
