- **A text field with its own right-click menu gets the caret where the click was.** A field's
  `on_secondary_press` (an app drawing its own Cut, Copy and Paste where there are no native
  menus, as on Linux) ran with the caret wherever it had been, so the menu's Paste landed
  somewhere else. The press now places the caret first, as the field's own menu does: in the
  selection it keeps it for Cut and Copy, elsewhere the caret moves to the click, and the field
  takes focus.
