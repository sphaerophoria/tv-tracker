"use strict";

// Used for both remote shows and full shows
export function render_card_element(show, href, extra_classes) {
  const card = document.createElement("div");
  card.classList.add("show-card");
  for (const klass of extra_classes) {
    card.classList.add(klass);
  }

  const link = document.createElement("a");
  link.href = href;
  card.appendChild(link);

  if (show.image !== null) {
    const poster = document.createElement("img");
    poster.src = show.image;
    link.appendChild(poster);
  } else {
    const poster = document.createElement("div");
    poster.classList.add("show-card-placeholder-image");
    link.appendChild(poster);
  }

  const name = document.createElement("p");
  name.innerText = show.name;
  if (show.year !== null) {
    name.innerText += " (" + show.year + ")";
  }
  name.classList.add("show-name");
  link.appendChild(name);

  return card;
}
