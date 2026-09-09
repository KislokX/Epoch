/** The visual Chronicle grammar is shared by every brain, so it has its own contract tests. */

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { RichChronicle } from "./RichChronicle";

function draw(content: string, opened: string[] = []) {
  return render(<RichChronicle content={content} onOpenLink={(url) => opened.push(url)} />);
}

describe("the rich Chronicle", () => {
  it("renders a safe common Markdown subset as headings, lists, emphasis and tables", () => {
    draw(`### Plan de compra

- **Revisar** el presupuesto
- Abrir [la guía](https://epoch.example/guide)

| Opción | Precio |
| --- | ---: |
| Usada | $450 |
| Nueva | $900 |`);

    expect(screen.getByRole("heading", { name: "Plan de compra" })).toBeInTheDocument();
    expect(screen.getByRole("list")).toHaveTextContent("Revisar el presupuesto");
    expect(screen.getByText("Revisar").tagName).toBe("STRONG");
    expect(screen.getByRole("table")).toHaveTextContent("Usada");
    expect(screen.getByRole("table")).toHaveTextContent("$900");
  });

  it("opens an explicit Markdown link through Epoch rather than by giving text browser powers", async () => {
    const user = userEvent.setup();
    const opened: string[] = [];
    draw("Consulta [la guía](https://epoch.example/guide).", opened);

    await user.click(screen.getByRole("button", { name: "la guía" }));
    expect(opened).toEqual(["https://epoch.example/guide"]);
  });

  it("draws a restricted Mermaid flow as boxes and arrows without interpreting it", () => {
    draw("```mermaid\nflowchart TD\nStart[Recibir pedido] --> Check{¿Hay stock?}\nCheck -->|sí| Ship[Enviar]\nCheck -->|no| Ask[Preguntar]\n```");

    expect(screen.getByRole("group", { name: "Flow diagram" })).toHaveTextContent("Recibir pedido");
    expect(screen.getByRole("group", { name: "Flow diagram" })).toHaveTextContent("¿Hay stock?");
    expect(screen.getByRole("group", { name: "Flow diagram" })).toHaveTextContent("sí");
    expect(screen.queryByText("flowchart TD")).not.toBeInTheDocument();
  });

  it("keeps code and HTML-shaped model text inert", () => {
    draw("```text\nhttps://epoch.example/not-a-link\n```\n\n<img src=x onerror=alert(1)>");

    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(document.querySelector("img")).toBeNull();
    expect(screen.getByText("<img src=x onerror=alert(1)>")).toBeInTheDocument();
  });

  it("does not lose a web address simply because a model emphasized it", async () => {
    const user = userEvent.setup();
    const opened: string[] = [];
    draw("**https://epoch.example/important**", opened);

    await user.click(screen.getByRole("button", { name: "https://epoch.example/important" }));
    expect(opened).toEqual(["https://epoch.example/important"]);
  });

  it("never turns a Markdown image into a picture or a link", () => {
    // **The one place a picture legitimately appears is `Entry::Produced`** — evidence the
    // Engine recorded because something really was made. A model that writes three characters
    // must not reach the same place, and a caption is not consent to open an address.
    const opened: string[] = [];
    render(
      <RichChronicle
        content={
          "Aqui tienes la imagen ![asuka](https://example.com/asuka.png) y " +
          "![otra](asuka.png)"
        }
        onOpenLink={(url) => opened.push(url)}
      />,
    );

    // No picture. This is the whole of the guarantee: a picture in a Chronicle comes from
    // `Entry::Produced` and from nowhere else.
    expect(document.querySelector("img")).toBeNull();
    // It stays what it is — text, with the syntax visible.
    expect(document.body.textContent).toContain("![asuka]");

    // **And no control captioned by the model.** A bare address still autolinks, which is the
    // ordinary behaviour for any URL somebody writes — but it now shows the address itself
    // rather than whatever word the model chose to hide it behind. That is the difference
    // between offering a link and disguising one.
    const links = [...document.querySelectorAll(".dlg__inline-link")];
    expect(links.map((it) => it.textContent)).not.toContain("asuka");
    for (const link of links) {
      expect(link.textContent).toContain("https://");
    }
  });

  it("still makes a link out of a link somebody wrote", () => {
    // The guard must not cost the ordinary case.
    const opened: string[] = [];
    render(
      <RichChronicle
        content="see [the docs](https://example.com/docs)"
        onOpenLink={(url) => opened.push(url)}
      />,
    );
    const link = document.querySelector(".dlg__inline-link");
    expect(link).not.toBeNull();
    expect(link?.textContent).toBe("the docs");
  });

});
