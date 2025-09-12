const evtSource = new EventSource('/events');
const list = document.getElementById('items-list');
const userForm = document.getElementById('userForm');
let uuid = '';

evtSource.addEventListener("init", (e) => {
    uuid = e.data;
});

userForm.addEventListener('submit', async (e) => {
    e.preventDefault();


    const userFormData = new FormData(userForm);

    if (uuid) {
        userFormData.append("client_uuid", uuid);
    }

    const params = new URLSearchParams();
    for (const [key, value] of userFormData.entries()) {
        params.append(key, value);
    }

    try {
        await fetch('/insertUser', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/x-www-form-urlencoded'
            },
            body: params.toString()
        });

        const getUpdatedUsers = await fetch('api/usernames');
        const userData = await getUpdatedUsers.json();
        const li = document.createElement('li');
        userData.usernames.forEach(username => {
            li.textContent = username;
            list.appendChild(li);
        });
        list.appendChild(li);
    } catch (err) {
        console.error(err);
    }
});

evtSource.onmessage = (e) => {
    console.log('sse:', e.data);
    if (e.data === 'new_post' || e.data === 'new_user') {
        const previousButton = document.getElementById("updateButton");
        if(previousButton) {
            return;
        }
        
        const li = document.createElement('li');
        const button = document.createElement('button');
        button.id = "updateButton";
        
        button.textContent = `New update: ${e.data}`;
        button.addEventListener('click', async () => {
            console.log("Button clicked:", e.data);

            const res = await fetch('/api/usernames');
            const data = await res.json();

            list.innerHTML = '';
            data.usernames.forEach(username => {
                const li = document.createElement('li');
                li.textContent = username;
                list.appendChild(li);
            });
        });

        li.appendChild(button);
        list.appendChild(li);
    }
};
