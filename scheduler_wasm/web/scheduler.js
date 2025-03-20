import init, { schedule_from_json } from "./scheduler_wasm.js";

// We'll keep tasks in a JavaScript array
let tasks = [
  { name: "Task A", windows: [{ Anchor: 540 }] },       // 9:00
  { name: "Lunch",  windows: [{ Anchor: 720 }] },       // 12:00
  { name: "Task B", windows: [{ Range: [780, 900] }] }  // 13:00–15:00
];

// Helper: parse "HH:MM" -> integer minutes
function parseHHMM(value) {
  const [hh, mm] = value.split(":").map(x => parseInt(x, 10));
  return hh * 60 + mm;
}

// Convert minutes back to "HH:MM"
function formatMinutes(m) {
  const hh = Math.floor(m / 60);
  const mm = m % 60;
  return `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;
}

// Render the task table
function renderTasks() {
  const tbody = document.querySelector("#task-table tbody");
  tbody.innerHTML = ""; // clear old rows

  tasks.forEach((task, idx) => {
    const tr = document.createElement("tr");

    // Row # / ID
    const tdNum = document.createElement("td");
    tdNum.textContent = idx + 1;
    tr.appendChild(tdNum);

    // Name
    const tdName = document.createElement("td");
    tdName.className = "editable";
    tdName.textContent = task.name;
    tdName.style.cursor = "pointer";
    // On click, open modal to edit name
    tdName.onclick = () => openEditModal("name", idx, task.name);
    tr.appendChild(tdName);

    // Window type
    const tdType = document.createElement("td");
    tdType.className = "editable";
    tdType.style.cursor = "pointer";
    tdType.onclick = () => openEditModal("type", idx, wtype);
    let wtype = "?";
    if (task.windows?.[0]?.Anchor !== undefined) {
      wtype = "Anchor";
    } else if (task.windows?.[0]?.Range !== undefined) {
      wtype = "Range";
    }
    tdType.textContent = wtype;
    tr.appendChild(tdType);

    // Time(s)
    const tdTime = document.createElement("td");
tdTime.className = "editable";
tdTime.style.cursor = "pointer";
tdTime.onclick = () => {
      if (wtype === "Anchor") {
        openEditModal("anchor-time", idx, formatMinutes(task.windows[0].Anchor));
      } else if (wtype === "Range") {
        const [start, end] = task.windows[0].Range;
        openEditModal("range-time", idx, `${formatMinutes(start)} - ${formatMinutes(end)}`);
      }
    };
    if (wtype === "Anchor") {
      const anchorVal = task.windows[0].Anchor;
      tdTime.textContent = formatMinutes(anchorVal);
    } else if (wtype === "Range") {
      const [startVal, endVal] = task.windows[0].Range;
      tdTime.textContent = `${formatMinutes(startVal)} - ${formatMinutes(endVal)}`;
    }
    tr.appendChild(tdTime);

    // Actions container with Up/Down/X:

    const tdActions = document.createElement("td");
    const actionsDiv = document.createElement("div");
actionsDiv.className = "row-actions";
    actionsDiv.style.display = "flex";
    actionsDiv.style.gap = "5px";

    // Up button (only if idx > 0, because the first item can't move up)
    if (idx > 0) {
      const upBtn = document.createElement("button");
      upBtn.textContent = "↑";
      upBtn.onclick = () => moveTask(idx, -1);
      actionsDiv.appendChild(upBtn);
    }

    // Down button (only if not the last item)
    if (idx < tasks.length - 1) {
      const downBtn = document.createElement("button");
      downBtn.textContent = "↓";
      downBtn.onclick = () => moveTask(idx, 1);
      actionsDiv.appendChild(downBtn);
    }

    // Remove button
    const removeBtn = document.createElement("button");
    removeBtn.textContent = "X";
    removeBtn.onclick = () => removeTask(idx);
    actionsDiv.appendChild(removeBtn);

    tdActions.appendChild(actionsDiv);
    tr.appendChild(tdActions);

    tbody.appendChild(tr);
  });
}

let currentEditingTask = { index: -1, field: "", value: "" };

function openEditModal(field, index, value) {
  currentEditingTask = { index, field, value };

  const modal = document.getElementById("edit-modal");
  const modalTitle = document.getElementById("edit-modal-title");
  const modalContent = document.getElementById("edit-modal-content");

  modalContent.innerHTML = "";  // clear previous contents

  if (field === "name") {
    modalTitle.textContent = "Edit Task Name";

    const input = document.createElement("input");
    input.type = "text";
    input.id = "edit-field-input";
    input.value = value;
    input.style.width = "100%";
    modalContent.appendChild(input);
  }
  else if (field === "type") {
    modalTitle.textContent = "Edit Window Type";

    const select = document.createElement("select");
    select.id = "edit-type-select";
    select.style.width = "100%";

    const anchorOption = document.createElement("option");
    anchorOption.value = "anchor";
    anchorOption.textContent = "Anchor";

    const rangeOption = document.createElement("option");
    rangeOption.value = "range";
    rangeOption.textContent = "Range";

    select.appendChild(anchorOption);
    select.appendChild(rangeOption);

    // Set current value
    const task = tasks[index];
    if (task.windows[0].Anchor !== undefined) {
      select.value = "anchor";
    } else {
      select.value = "range";
    }

    modalContent.appendChild(select);
  }
  else if (field === "anchor-time") {
    modalTitle.textContent = "Edit Anchor Time";

    const input = document.createElement("input");
    input.type = "text";
    input.id = "edit-time-input";
    input.value = value;
    input.pattern = "\\d{1,2}:\\d{2}";
    input.title = "Format: HH:MM";
    input.style.width = "100%";

    modalContent.appendChild(input);
  }
  else if (field === "range-time") {
    modalTitle.textContent = "Edit Time Range";

    const [start, end] = value.split(" - ");

    const startLabel = document.createElement("label");
    startLabel.textContent = "Start:";

    const startInput = document.createElement("input");
    startInput.type = "text";
    startInput.id = "edit-start-input";
    startInput.value = start;
    startInput.pattern = "\\d{1,2}:\\d{2}";
    startInput.title = "Format: HH:MM";
    startInput.style.width = "100%";

    const endLabel = document.createElement("label");
    endLabel.textContent = "End:";
    endLabel.style.marginTop = "10px";

    const endInput = document.createElement("input");
    endInput.type = "text";
    endInput.id = "edit-end-input";
    endInput.value = end;
    endInput.pattern = "\\d{1,2}:\\d{2}";
    endInput.title = "Format: HH:MM";
    endInput.style.width = "100%";

    modalContent.appendChild(startLabel);
    modalContent.appendChild(startInput);
    modalContent.appendChild(endLabel);
    modalContent.appendChild(endInput);
  }

  modal.style.display = "block";
}

function closeModal() {
  document.getElementById("edit-modal").style.display = "none";
}

function saveModalChanges() {
  const { index, field } = currentEditingTask;
  const task = tasks[index];

  if (field === "name") {
    const newValue = document.getElementById("edit-field-input").value.trim();
    if (newValue) {
      task.name = newValue;
    }
  }
  else if (field === "type") {
    const newType = document.getElementById("edit-type-select").value;

    // Convert the window type
    if (newType === "anchor") {
      // If switching from range to anchor, use the midpoint
      if (task.windows[0].Range) {
        const [start, end] = task.windows[0].Range;
        const midpoint = Math.floor((start + end) / 2);
        task.windows[0] = { Anchor: midpoint };
      }
    }
    else if (newType === "range") {
      // If switching from anchor to range, create a range around it
      if (task.windows[0].Anchor) {
        const anchor = task.windows[0].Anchor;
        task.windows[0] = { Range: [Math.max(0, anchor - 60), anchor + 60] };
      }
    }
  }
  else if (field === "anchor-time") {
    const timeStr = document.getElementById("edit-time-input").value.trim();
    if (timeStr.match(/^\d{1,2}:\d{2}$/)) {
      task.windows[0].Anchor = parseHHMM(timeStr);
    } else {
      alert("Please use HH:MM format");
      return;
    }
  }
  else if (field === "range-time") {
    const startStr = document.getElementById("edit-start-input").value.trim();
    const endStr = document.getElementById("edit-end-input").value.trim();

    if (!startStr.match(/^\d{1,2}:\d{2}$/) || !endStr.match(/^\d{1,2}:\d{2}$/)) {
      alert("Please use HH:MM format");
      return;
    }

    const startMin = parseHHMM(startStr);
    const endMin = parseHHMM(endStr);

    if (endMin <= startMin) {
      alert("End time must be later than start time");
      return;
    }

    task.windows[0].Range = [startMin, endMin];
  }

  // Close the modal and re-render
  closeModal();
  renderTasks();
}

function moveTask(index, direction) {
  // 'direction' is +1 (move down) or -1 (move up)
  const newIndex = index + direction;
  if (newIndex >= 0 && newIndex < tasks.length) {
    // Swap the tasks
    [tasks[index], tasks[newIndex]] = [tasks[newIndex], tasks[index]];
    // Re-render
    renderTasks();
  }
}

// Remove a task by index
function removeTask(index) {
  tasks.splice(index, 1);
  renderTasks();
}

// Add a new task based on form
function addTask() {
  const nameField = document.getElementById("task-name");
  const nameVal = nameField.value.trim();
  if (!nameVal) {
    alert("Please enter a task name.");
    return;
  }

  const wtype = document.getElementById("window-type").value;

  if (wtype === "anchor") {
    const anchorVal = document.getElementById("anchor-time").value.trim();
    if (!anchorVal.match(/^\d{1,2}:\d{2}$/)) {
      alert("Anchor time must be in HH:MM format.");
      return;
    }
    const anchorMin = parseHHMM(anchorVal);

    tasks.push({
      name: nameVal,
      windows: [ { Anchor: anchorMin } ]
    });

  } else if (wtype === "range") {
    const startVal = document.getElementById("range-start").value.trim();
    const endVal   = document.getElementById("range-end").value.trim();

    if (!startVal.match(/^\d{1,2}:\d{2}$/) || !endVal.match(/^\d{1,2}:\d{2}$/)) {
      alert("Range times must be in HH:MM format.");
      return;
    }
    const startMin = parseHHMM(startVal);
    const endMin   = parseHHMM(endVal);
    if (endMin <= startMin) {
      alert("End time must be > start time.");
      return;
    }

    tasks.push({
      name: nameVal,
      windows: [ { Range: [startMin, endMin] } ]
    });
  }

  // Clear fields
  nameField.value = "";
  document.getElementById("anchor-time").value = "09:00";
  document.getElementById("range-start").value = "13:00";
  document.getElementById("range-end").value   = "15:00";

  // Re-render table
  renderTasks();
}

// Render the final schedule result in a table
function renderScheduleResult(scheduleData) {
  const scheduleTable = document.getElementById("schedule-table");
  const tbody = scheduleTable.querySelector("tbody");
  tbody.innerHTML = "";

  scheduleData.forEach((item, idx) => {
    // item is [ taskName, minutesFloat ]
    const tr = document.createElement("tr");

    // # column
    const tdNum = document.createElement("td");
    tdNum.textContent = idx + 1;
    tr.appendChild(tdNum);

    // Task name
    const tdName = document.createElement("td");
    tdName.textContent = item[0] || "???";
    tr.appendChild(tdName);

    // Time
    const tdTime = document.createElement("td");
    const rawMinutes = item[1] || 0;
    // Convert f64 to integer
    const minutes = Math.round(rawMinutes);
    tdTime.textContent = formatMinutes(minutes);
    tr.appendChild(tdTime);

    tbody.appendChild(tr);
  });

  // Show the table
  scheduleTable.style.display = "table";
}

function solveSchedule() {
  // Get day start and end times
  const dayStartElem = document.getElementById("day-start");
  const dayEndElem = document.getElementById("day-end");

  // Validate inputs
  if (!dayStartElem.value.match(/^\d{1,2}:\d{2}$/) || !dayEndElem.value.match(/^\d{1,2}:\d{2}$/)) {
    alert("Day start and end times must be in HH:MM format");
    return;
  }

  const dayStartMin = parseHHMM(dayStartElem.value);
  const dayEndMin = parseHHMM(dayEndElem.value);

  if (dayEndMin <= dayStartMin) {
    alert("Day end time must be after day start time");
    return;
  }

  // Build the config object
  const config = {
    tasks: tasks,
    dayStart: dayStartMin,
    dayEnd: dayEndMin
  };

  // Convert to JSON string
  const jsonInput = JSON.stringify(config);

  // Call the WASM schedule function
  const result = schedule_from_json(jsonInput);

  // Check if the result is valid JSON
  let scheduleData = null;
  const outputEl = document.getElementById("output");
  outputEl.classList.remove("error");
  outputEl.textContent = "";

  try {
    // Attempt to parse as JSON => e.g. [ ["Task A", 480.0], ... ]
    scheduleData = JSON.parse(result);
  } catch (err) {
    // It's not valid JSON => an error message
    outputEl.classList.add("error");
    outputEl.textContent = result;
    // Hide schedule table
    document.getElementById("schedule-table").style.display = "none";
    return;
  }

  // If we got here, it's parsed successfully. Render the table of results
  renderScheduleResult(scheduleData);
}

async function run() {
  await init(); // Load WASM

  // Set up form toggling (anchor vs range)
  const windowTypeEl = document.getElementById("window-type");
  windowTypeEl.onchange = () => {
    const typeVal = windowTypeEl.value;
    document.getElementById("anchor-inputs").style.display = (typeVal === "anchor") ? "block" : "none";
    document.getElementById("range-inputs").style.display  = (typeVal === "range")  ? "block" : "none";
  };

  // Hook up button events
  document.getElementById("btn-add-task").onclick = addTask;
  document.getElementById("btn-schedule").onclick = solveSchedule;
  document.getElementById("edit-modal-save").onclick = saveModalChanges;
  document.getElementById("edit-modal-cancel").onclick = closeModal;

  // Render initial table
  renderTasks();
}

run();
