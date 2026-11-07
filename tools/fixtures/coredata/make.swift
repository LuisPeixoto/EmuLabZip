import CoreData
import Foundation

// A model with two entities made in code, saved with the SQLite store: the layout Core Data writes on a device.
let person = NSEntityDescription()
person.name = "Person"
person.managedObjectClassName = "NSManagedObject"
let name = NSAttributeDescription(); name.name = "name"; name.attributeType = .stringAttributeType; name.isOptional = false
let age = NSAttributeDescription(); age.name = "age"; age.attributeType = .integer16AttributeType
let score = NSAttributeDescription(); score.name = "score"; score.attributeType = .doubleAttributeType
let born = NSAttributeDescription(); born.name = "bornAt"; born.attributeType = .dateAttributeType

let pet = NSEntityDescription()
pet.name = "Pet"
pet.managedObjectClassName = "NSManagedObject"
let petName = NSAttributeDescription(); petName.name = "nickname"; petName.attributeType = .stringAttributeType

let owner = NSRelationshipDescription(); owner.name = "owner"; owner.destinationEntity = person; owner.minCount = 0; owner.maxCount = 1
let pets = NSRelationshipDescription(); pets.name = "pets"; pets.destinationEntity = pet; pets.minCount = 0; pets.maxCount = 0
owner.inverseRelationship = pets; pets.inverseRelationship = owner
person.properties = [name, age, score, born, pets]
pet.properties = [petName, owner]

let model = NSManagedObjectModel()
model.entities = [person, pet]
let url = URL(fileURLWithPath: CommandLine.arguments[1])
try? FileManager.default.removeItem(at: url)
let coordinator = NSPersistentStoreCoordinator(managedObjectModel: model)
try coordinator.addPersistentStore(ofType: NSSQLiteStoreType, configurationName: nil, at: url, options: [NSSQLitePragmasOption: ["journal_mode": "DELETE"]])
let context = NSManagedObjectContext(concurrencyType: .mainQueueConcurrencyType)
context.persistentStoreCoordinator = coordinator
for (n, a, s) in [("Ana", 31, 9.5), ("Bo", 27, 7.25)] {
    let p = NSEntityDescription.insertNewObject(forEntityName: "Person", into: context)
    p.setValue(n, forKey: "name"); p.setValue(a, forKey: "age"); p.setValue(s, forKey: "score"); p.setValue(Date(timeIntervalSince1970: 631152000), forKey: "bornAt")
    let d = NSEntityDescription.insertNewObject(forEntityName: "Pet", into: context)
    d.setValue(n + "'s cat", forKey: "nickname"); d.setValue(p, forKey: "owner")
}
try context.save()
print("saved")
