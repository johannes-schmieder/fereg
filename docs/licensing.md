# Dependency and native-distribution decision

Johannes approved retaining the pinned within and GPL-3.0-only CMG dependencies
and accepts a GPLv3 distribution direction for fereg. Preserve third-party terms;
one rights holder's license does not grant exceptions for another's code.

Before distributing native Stata plugins, resolve the Stata/SPI combination's
licensing requirements and obtain any necessary permissions. Project licensing,
Cargo metadata, third-party notices and corresponding-source packaging must be
made consistent with the resolved distribution decision. That work and permission
remain outstanding; changing a license label alone is insufficient.

Development, local validation, Git pushes and review packages do not authorize
publication of native release assets. Current dependency revisions are recorded
in Cargo.lock and references.lock.toml. Historical decision text is retained in Git.
