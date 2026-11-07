// Writes the Realm file the reader in crates/emulabzip-storage is tested against, with the official `realm` package:
//
//   mkdir /tmp/realm-fixture && cd /tmp/realm-fixture && npm init -y && npm install realm
//   node <repo>/tools/fixtures/realm/make.mjs default.realm
import Realm from "realm";

const Person = {
  name: "Person",
  primaryKey: "id",
  properties: {
    id: "int",
    name: "string",
    age: "int?",
    score: "double",
    ratio: "float",
    active: "bool",
    born: "date",
    avatar: "data?",
    token: "objectId",
    note: "string?",
    // 16 to 63 characters: the middle size of string the file format stores.
    memo: "string?",
    tags: "string[]",
    friend: "Person?",
  },
};
const Pet = { name: "Pet", properties: { nickname: "string", legs: "int", owner: "Person?" } };
// More rows than one cluster holds (256), so the rows sit in a tree.
const Big = { name: "Big", properties: { n: "int", label: "string", wide: "double?" } };

const path = process.argv[2] ?? "default.realm";
Realm.deleteFile({ path });
const realm = new Realm({ path, schema: [Person, Pet, Big], schemaVersion: 1 });
realm.write(() => {
  const ana = realm.create("Person", {
    id: 1, name: "Ana", age: 31, score: 9.5, ratio: 0.25, active: true, born: new Date("1990-01-01T00:00:00Z"),
    avatar: new Uint8Array([1, 2, 3, 4]).buffer, token: new Realm.BSON.ObjectId("65f0a1b2c3d4e5f607182930"),
    note: "first user with a longer note that does not fit in a short string slot of the array format", memo: "a memo of forty characters, more or less", tags: ["a", "bb"],
  });
  realm.create("Person", {
    id: 2, name: "Bo", age: null, score: -1.5, ratio: 1.5, active: false, born: new Date("2001-09-09T01:46:40Z"),
    token: new Realm.BSON.ObjectId("65f0a1b2c3d4e5f607182931"), note: null, memo: "short", tags: [], friend: ana,
  });
  realm.create("Person", {
    id: 3, name: "Cy", age: 0, score: 0, ratio: 0, active: true, born: new Date("1969-12-31T23:59:59Z"),
    token: new Realm.BSON.ObjectId("65f0a1b2c3d4e5f607182932"), note: "", memo: null, tags: ["x"],
  });
  realm.create("Pet", { nickname: "Tom", legs: 4, owner: ana });
  realm.create("Pet", { nickname: "Tweety", legs: 2, owner: null });
  for (let i = 0; i < 700; i++) realm.create("Big", { n: i * 3 - 5, label: `row ${i}`, wide: i % 7 === 0 ? null : i / 4 });
});
realm.close();
console.log("wrote", path);
// The package keeps the process alive after the file is closed.
process.exit(0);
