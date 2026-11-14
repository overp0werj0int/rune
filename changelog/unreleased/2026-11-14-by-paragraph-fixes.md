- **A by-paragraph field reads right to a screen reader and types fast with many spans.** A
  blank paragraph is shaped with a stand-in character for its height, and its accessible line
  ran one byte past its end, so the next paragraph's first character was read twice
  ("##draft"). Its lines now end where the paragraph does. Laying out a field with
  `dir_by_paragraph()` also checked every span against every paragraph; with sorted spans it
  now looks only at the spans that can reach the paragraph, so a 3,000-line source editor with
  about 6,000 spans takes a keystroke in about 4 ms instead of 17.
