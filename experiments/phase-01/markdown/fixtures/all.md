# StickyMD Phase 1D fixture

CommonMark + GFM + math (all four delimiters) + raw HTML literal.

## Inline math (dollar)

Einstein: $E = mc^2$ and the quadratic $x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$.

## Display math (double dollar)

$$
\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}
$$

## Inline math (backslash-paren)

The identity \( e^{i\pi} + 1 = 0 \) is beautiful.

## Display math (backslash-bracket)

\[
\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}
\]

## GFM features

| col A | col B |
| ----- | ----- |
| 1     | 2     |

- [x] task done
- [ ] task open

```rust
fn main() { println!("code block stays literal"); }
```

## Raw HTML (must stay literal, not parsed as markdown)

<div class="raw-html">
  <span data-x="1">this is raw HTML, kept as literal</span>
</div>

Some **bold**, *italic*, `code span`, and a [link](https://example.com).
