import { get_ratings, put_rating, put_ratings, delete_rating } from "./http.js";

class RatingsPage {
  constructor(ratings) {
    this.ratings = ratings;
  }

  async update_rating_name(rating_id, item) {
    const new_rating = window.structuredClone(this.ratings[rating_id]);
    new_rating.name = item.innerText;
    this.ratings[rating_id] = await put_rating(new_rating);
    this.render();
  }

  async swap_priorities(a, b) {
    let a_new = window.structuredClone(a);
    let b_new = window.structuredClone(b);

    let tmp = a_new.priority;
    a_new.priority = b_new.priority;
    b_new.priority = tmp;

    this.ratings[a_new.id] = await put_rating(a_new);
    this.ratings[b_new.id] = await put_rating(b_new);

    this.render();
  }

  async add_rating(name) {
    let response = await put_ratings({
      name: name,
    });
    this.ratings[response.id] = response;
    this.render();
  }

  async delete_rating(rating_id) {
    await delete_rating(rating_id);
    delete this.ratings[rating_id];
    this.render();
  }

  render() {
    let ratings = Object.values(this.ratings);
    ratings.sort((a, b) => a.priority >= b.priority);

    let div = document.getElementById("ratings-div");
    div.innerHTML = "";

    for (const rating of ratings) {
      const elem_div = document.createElement("div");
      elem_div.classList.add("rating-div");
      div.appendChild(elem_div);

      add_rating_to_div(this, rating, elem_div);
      add_up_button_to_div(this, rating, elem_div);
      add_down_button_to_div(this, rating, elem_div);
      add_delete_button_to_div(this, rating, elem_div);
    }
  }
}

function add_rating_to_div(page, rating, elem_div) {
  const number = document.createElement("h2");
  number.innerHTML = "" + rating.priority + ".";
  elem_div.appendChild(number);

  const header = document.createElement("h2");
  header.contentEditable = true;
  header.innerHTML = rating.name;

  header.onkeydown = (event) => {
    if (event.keyCode == 13) {
      event.preventDefault();
      header.blur();
    }
  };

  header.onblur = (event) => {
    page.update_rating_name(rating.id, header);
  };

  elem_div.appendChild(header);
}

function add_up_button_to_div(page, rating, elem_div) {
  const up_button = document.createElement("input");
  up_button.type = "button";
  up_button.value = "Up";
  up_button.onclick = () => {
    let ratings = Object.values(page.ratings);
    let target_item = find_lower_priority(ratings, rating.priority);
    if (target_item !== null) {
      page.swap_priorities(rating, target_item);
    }
  };
  elem_div.appendChild(up_button);
}

function add_down_button_to_div(page, rating, elem_div) {
  const down_button = document.createElement("input");
  down_button.type = "button";
  down_button.value = "Down";
  down_button.onclick = () => {
    let ratings = Object.values(page.ratings);
    let target_item = find_higher_priority(ratings, rating.priority);
    if (target_item !== null) {
      page.swap_priorities(rating, target_item);
    }
  };
  elem_div.appendChild(down_button);
}

function add_delete_button_to_div(page, rating, elem_div) {
  const delete_button = document.createElement("input");
  delete_button.type = "button";
  delete_button.value = "Delete";
  delete_button.onclick = () => {
    page.delete_rating(rating.id);
  };
  elem_div.appendChild(delete_button);
}

function find_closest_with_thresh(array, thresh, cmp) {
  let ret = array.reduce((acc, current) => {
    // In the case of trying to find the largest value below the threshold
    // cmp is >

    // We negate the comparison to get the equivalent of <= from >
    // If thresh <= current.priority we skip current
    if (!cmp(current.priority, thresh)) {
      return acc;
    }

    // At this point we are below the threshold

    // If the accumulator is null, no work to do, this is the first valid option
    if (acc === null) {
      return current;
    }

    // Otherwise we pick the larger value between the accumulator and current item
    if (cmp(acc.priority, current.priority)) {
      return current;
    }

    return acc;
  }, null);

  return ret;
}

function find_lower_priority(array, target_priority) {
  return find_closest_with_thresh(array, target_priority, (a, b) => a < b);
}

function find_higher_priority(array, target_priority) {
  return find_closest_with_thresh(array, target_priority, (a, b) => a > b);
}

async function add_rating() {
  const add_rating_input = document.getElementById("add-rating-text");
  await put_ratings({
    name: add_rating_input.value,
  });
  add_rating_input.value = "";
  init();
}

async function init() {
  const ratings = await get_ratings();
  const page = new RatingsPage(ratings);
  page.render();

  const add_button = document.getElementById("add-button");
  add_button.onclick = add_rating;
}

init();
