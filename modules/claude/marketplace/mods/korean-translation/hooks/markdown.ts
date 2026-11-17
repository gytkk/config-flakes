// A block-structure check, not a CommonMark parser or a semantic completeness check.
export function markdownShape(document: string): string[] {
  const shape: string[] = []
  let fence: string | undefined
  let quoteDepth = 0
  let table = false
  let previous = ''
  for (const raw of document.split(/\r?\n/)) {
    const indent = raw.match(/^[ \t]*/)![0].replaceAll('\t', '    ').length
    let line = raw.trimStart()
    let depth = 0
    while (line.startsWith('>')) { depth++; line = line.slice(1).replace(/^[ \t]?/, '') }
    if (fence) {
      if (line.trim().length >= fence.length && [...line.trim()].every(char => char === fence![0])) fence = undefined
      continue
    }
    if (depth !== quoteDepth) {
      if (depth) shape.push(`quote:${depth}`)
      quoteDepth = depth
    }
    const opening = /^(`{3,}|~{3,})/.exec(line)
    if (opening) {
      fence = opening[1]
      shape.push(`code:${depth}:${indent}`)
      previous = ''
      continue
    }
    const context = `${depth}:${indent}`
    // Pipes inside inline code or escaped in prose are not table separators.
    const plain = line.replace(/(`+).*?\1/g, 'code').trim()
    const cells = plain.replace(/^\|/, '').replace(/(?<!\\)\|$/, '').split(/(?<!\\)\|/).map(cell => cell.trim())
    const separator = cells.length > 1 && cells.every(cell => /^:?-{3,}:?$/.test(cell))
    if (separator) {
      shape.push(`table:${context}:header=${previous.replace(/^\|/, '').replace(/\|\s*$/, '').split(/(?<!\\)\|/).length}:${cells.join('|')}`)
      table = true
    } else if (table && cells.length > 1) {
      shape.push(`row:${context}:${cells.length}`)
    } else {
      table = false
      const heading = /^(#{1,6})(?:[ \t]|$)/.exec(line)
      const list = /^(?:([-+*])|(\d{1,9}[.)]))[ \t]+(?:\[([ xX])\][ \t]+)?/.exec(line)
      if (heading) shape.push(`heading:${context}:${heading[1]!.length}`)
      else if (/^(?:={3,}|-{3,})\s*$/.test(line) && previous.trim() && !/^[-+*]\s|^\d+[.)]\s/.test(previous)) {
        shape.push(`setext:${context}:${line[0]}`)
      } else if (/^([-*_])(?:[ \t]*\1){2,}[ \t]*$/.test(line)) shape.push(`rule:${context}`)
      else if (list) shape.push(`list:${context}:${list[1] ?? list[2]}:${list[3]?.toLowerCase() ?? ''}`)
    }
    previous = line
  }
  return shape
}
