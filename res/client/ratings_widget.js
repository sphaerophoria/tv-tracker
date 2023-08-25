export function create_ratings_selector(item, ratings, parent, onchange) {
  const label = document.createElement("label");
  const ratings_selector = document.createElement("select");
  label.innerText = "Rating:";

  const no_rating_option = document.createElement("option");
  no_rating_option.rating_id = null;
  no_rating_option.innerText = "None";
  ratings_selector.add(no_rating_option);

  ratings = Object.values(ratings).sort((a, b) => a.priority - b.priority);

  for (const rating of ratings) {
    let option = document.createElement("option");
    option.rating_id = rating.id;
    option.innerText = rating.name;
    ratings_selector.add(option);

    if (rating.id == item.rating_id) {
      ratings_selector.selectedIndex = ratings_selector.length - 1;
    }
  }

  ratings_selector.onchange = onchange;
  console.log(ratings_selector);

  parent.appendChild(label);
  parent.appendChild(ratings_selector);
}
