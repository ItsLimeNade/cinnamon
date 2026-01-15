import { Cinnamon, DeviceType, Trend } from 'cinnamon-rs';

const NS_URL = process.env.NS_URL || "https://www.your-ns-url.com/";
const NS_SECRET = process.env.NS_SECRET || "your-ns-pass";

async function main() {
    console.log(`Connecting to: ${NS_URL}`);
    
    try {
        const client = new Cinnamon(NS_URL, NS_SECRET);

        console.log("Fetching SGV entries");
        const entries = await client.sgv()
        .limit(5)
        .filterDevice(DeviceType.Auto) 
        .fetch();

        console.log(`Found ${entries.length} entries.`);
        
        if (entries.length > 0) {
        const latest = entries[0];
        console.log(`Latest SGV: ${latest.sgv} mg/dL`);
        console.log(`Direction: ${latest.direction}`);
        
        if (latest.direction === Trend.DoubleUp) {
            console.log("Rising fast!");
        }
        }

        console.log("Fetching System Properties");
        const props = await client.properties()
        .enable(["iob", "cob", "pump"])
        .fetch();
        
        if (props.iob) {
        console.log(`IOB: ${props.iob.iob.toFixed(2)} U`);
        }
        if (props.cob) {
        console.log(`COB: ${props.cob.cob.toFixed(1)} g`);
        }

    } catch (error) {
        console.error("Error:", error);
    }
}

main();