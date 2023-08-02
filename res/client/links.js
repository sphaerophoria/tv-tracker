async function init() {
  const response = await fetch("/links.html");
  const response_body = await response.text();
  const div = document.createElement("div");
  div.innerHTML = response_body;
  document.body.prepend(div);
}

init();
