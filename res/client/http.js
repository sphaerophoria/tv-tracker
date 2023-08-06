export async function request_episodes(show_id) {
  const params = new URLSearchParams({ show_id: show_id });
  const request = new Request("/episodes?" + params.toString(), {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function request_shows() {
  const request = new Request("/shows", {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}
