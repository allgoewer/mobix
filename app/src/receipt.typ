// Receipt template. The app writes the message parts next to this file and
// passes their names via `--input`; text is read from a file so it never has
// to be escaped (and is not limited by the command line length).

#set page(width: 80mm, height: auto, margin: 4mm, fill: white)
#set text(size: 10pt)
#set par(justify: false)

#let image-file = sys.inputs.at("image", default: none)
#let text-file = sys.inputs.at("text", default: none)

#if image-file != none {
  align(center, image(image-file, width: 100%))
}

#if text-file != none {
  let body = read(text-file)
  if sys.inputs.at("markup", default: "0") == "1" {
    eval(body, mode: "markup")
  } else {
    // Keep the sender's line breaks
    body.split("\n").join(linebreak())
  }
}
