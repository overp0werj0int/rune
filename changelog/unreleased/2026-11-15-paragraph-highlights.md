- **A keyed highlight in a by-paragraph field can be scrolled to.** `Tx::scroll_key_to` and
  `Tx::scroll_center` on a `TextHighlight` key found the highlight's boxes only in a text laid
  out whole, so in a field with `dir_by_paragraph()` they did nothing. The boxes now come from
  the paragraph that holds the highlight.
