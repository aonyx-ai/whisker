# Prebuilt archives

<!--
goal: let a consumer read what happened when whisker did or did not use
published archives.
non-goal: how to publish them. that is the reference page of the same name in
the authoring tree.
-->

## the lookup order

<!--
when does whisker even ask a release. only when the cache holds nothing for the
entry, so a warm checkout keeps compiling after rules start publishing.
how do I make it ask again. move the pin, or clear the cache.
-->

## what it says

<!--
why did I see no message. a table of cases, most silent, the rest one line on
stderr. every one of them falls back to compiling the source.
why did a handshake failure end my run instead. the publisher named the archive
with a tag that does not describe it, and a quiet compile would hide that from
everyone who trusts the tag.
-->

| Case                                                | Output                               |
| --------------------------------------------------- | ------------------------------------ |
| No release names an archive for this tag            | Nothing. Whisker compiles the source |
| The remote is not on GitHub, or the API answers 404 | Nothing                              |
| An API Whisker cannot reach, or an error answer     | One line on stderr                   |
| A digest that does not match                        | One line on stderr                   |
| An archive that will not unpack                     | One line on stderr                   |
| A cache directory Whisker cannot write              | One line on stderr                   |
