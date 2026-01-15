const { Cinnamon, DeviceType } = require('cinnamon-rs');

const NS_URL = process.env.NS_URL || "https://www.your-ns-url.com/";
const NS_SECRET = process.env.NS_SECRET || "your-ns-pass";

async function main() {
    console.log(`Connecting to: ${NS_URL}`);

    try {
        const client = new Cinnamon(NS_URL, NS_SECRET);

        console.log("Fetching Treatments.");
        const treatments = await client.treatments()
        .limit(3)
        .filterDevice(DeviceType.All)
        .fetch();

        console.log(`Found ${treatments.length} treatments.`);
        
        treatments.forEach((t, i) => {
        console.log(`[${i + 1}] ${t.eventType} at ${t.createdAt}`);
        if (t.insulin) console.log(`Insulin: ${t.insulin} U`);
        if (t.carbs) console.log(`Carbs: ${t.carbs} g`);
        });

    } catch (error) {
        console.error("Error:", error);
    }
}

main();