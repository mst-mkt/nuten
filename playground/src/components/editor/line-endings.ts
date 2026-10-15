import { invertedEffects } from "@codemirror/commands";
import {
  type ChangeDesc,
  ChangeSet,
  EditorState,
  StateEffect,
  StateField,
  type Transaction,
} from "@codemirror/state";
import { Decoration, EditorView, WidgetType, highlightSpecialChars } from "@codemirror/view";

const carriageReturn = 0x0d;

const symbolOf = (title: string) => {
  return Object.assign(document.createElement("span"), {
    className: "cm-specialChar",
    textContent: "␍",
    title,
  });
};

export const splitLineEndings = (text: string) => {
  const doc = text.replaceAll("\r\n", "\n");
  const crlf = Array.from(text.matchAll(/\r\n/g), (match, index) => match.index - index);
  return { doc, crlf };
};

export const joinLineEndings = (doc: string, crlf: readonly number[]) => {
  const starts = [0, ...crlf];
  const ends = [...crlf, doc.length];
  return starts.map((start, index) => doc.slice(start, ends[index])).join("\r");
};

const crlfAdded = StateEffect.define<readonly number[]>({
  map: (positions, changes) => positions.map((pos) => changes.mapPos(pos, 1)),
});

const breakMappedThrough = (changes: ChangeDesc, pos: number) => {
  const from = changes.mapPos(pos, 1);
  const to = changes.mapPos(pos + 1, -1);
  return to - from === 1 ? [from] : [];
};

class CrlfWidget extends WidgetType {
  override eq() {
    return true;
  }

  override toDOM() {
    return symbolOf("CRLF");
  }
}

const crlfSymbol = Decoration.widget({ widget: new CrlfWidget(), side: 1 });

const crlfBreaks = StateField.define<readonly number[]>({
  create: () => [],
  update: (breaks, tr) => {
    const kept = tr.docChanged
      ? breaks.flatMap((pos) => breakMappedThrough(tr.changes, pos))
      : breaks;
    const marked = tr.effects.flatMap((effect) => (effect.is(crlfAdded) ? effect.value : []));
    const added = marked.filter((pos) => tr.newDoc.sliceString(pos, pos + 1) === "\n");
    if (added.length === 0) return kept;

    return [...new Set([...kept, ...added])].sort((a, b) => a - b);
  },
  provide: (field) => {
    return EditorView.decorations.from(field, (breaks) => {
      return Decoration.set(breaks.map((pos) => crlfSymbol.range(pos)));
    });
  },
});

export const textOf = (state: EditorState) => {
  return joinLineEndings(state.doc.toString(), state.field(crlfBreaks));
};

const sliceOf = (state: EditorState, from: number, to: number) => {
  const crlf = state.field(crlfBreaks).filter((pos) => pos >= from && pos < to);
  const relative = crlf.map((pos) => pos - from);
  return joinLineEndings(state.sliceDoc(from, to), relative);
};

const endsWithCrlf = (state: EditorState, line: number) => {
  if (line < 1 || line >= state.doc.lines) return false;

  return state.field(crlfBreaks).includes(state.doc.line(line).to);
};

const takesCrlf = (state: EditorState, line: number) => {
  return endsWithCrlf(state, line < state.doc.lines ? line : line - 1);
};

type Insertion = { fromA: number; fromB: number; text: string };

const insertionsOf = (tr: Transaction) => {
  const insertions: Insertion[] = [];
  tr.changes.iterChanges((fromA, _toA, fromB, _toB, inserted) => {
    insertions.push({ fromA, fromB, text: inserted.toString() });
  });
  return insertions;
};

const carriageReturnsIn = ({ fromB, text }: Insertion) => {
  return Array.from(text.matchAll(/\r\n/g), (match) => fromB + match.index);
};

const writtenBreaksIn = (state: EditorState, { fromA, fromB, text }: Insertion) => {
  if (!takesCrlf(state, state.doc.lineAt(fromA).number)) return [];

  return Array.from(text.matchAll(/(?<!\r)\n/g), (match) => fromB + match.index);
};

const writesLineBreaks = (tr: Transaction) => {
  const inserted = tr.isUserEvent("input.paste") || tr.isUserEvent("input.drop");
  const typed = tr.isUserEvent("input") && !inserted;
  return typed || tr.isUserEvent("move.line");
};

const crlfTakenOut = EditorState.transactionFilter.of((tr) => {
  if (!tr.docChanged) return tr;

  const insertions = insertionsOf(tr);
  const carriageReturns = insertions.flatMap(carriageReturnsIn);
  const written = writesLineBreaks(tr)
    ? insertions.flatMap((insertion) => writtenBreaksIn(tr.startState, insertion))
    : [];
  if (carriageReturns.length === 0 && written.length === 0) return tr;

  const removed = carriageReturns.map((from) => ({ from, to: from + 1 }));
  const removal = ChangeSet.of(removed, tr.newDoc.length);
  const breaks = [...carriageReturns.map((pos) => pos + 1), ...written];
  const effects = crlfAdded.of(breaks.map((pos) => removal.mapPos(pos)));
  return [tr, { changes: removal, effects, sequential: true }];
});

const crlfRestoredByHistory = invertedEffects.of((tr) => {
  if (!tr.docChanged) return [];

  const breaks = tr.startState.field(crlfBreaks);
  const dropped = breaks.filter((pos) => breakMappedThrough(tr.changes, pos).length === 0);
  return dropped.length === 0 ? [] : [crlfAdded.of(dropped)];
});

const pastedLinePerCursor = EditorView.clipboardInputFilter.of((text, state) => {
  const lines = text.split(/\r?\n/);
  const perCursor =
    state.selection.ranges.length > 1 && lines.length === state.selection.ranges.length;
  return perCursor ? lines.join("\n") : text;
});

const copiedWithCrlf = EditorView.clipboardOutputFilter.of((text, state) => {
  const ranges = state.selection.ranges.filter((range) => !range.empty);
  const plain = ranges.map(({ from, to }) => state.sliceDoc(from, to)).join(state.lineBreak);
  if (ranges.length === 0 || plain !== text) return text;

  return ranges.map(({ from, to }) => sliceOf(state, from, to)).join(state.lineBreak);
});

const loneCarriageReturns = highlightSpecialChars({
  render: (code, description, placeholder) => {
    if (code === carriageReturn) return symbolOf(description ?? "");

    return Object.assign(document.createElement("span"), {
      className: "cm-specialChar",
      textContent: placeholder,
      title: description ?? "",
    });
  },
});

export const lineEndings = (crlf: readonly number[]) => [
  EditorState.lineSeparator.of("\n"),
  crlfBreaks.init(() => crlf),
  crlfTakenOut,
  crlfRestoredByHistory,
  pastedLinePerCursor,
  copiedWithCrlf,
  loneCarriageReturns,
];
