import React, {useState} from 'react';
import {Button, FlatList, SafeAreaView, StyleSheet, Switch, Text, View} from 'react-native';

// A small screen to try EmuLabZip's React Native tools on: nested components, props, a counter and a list.
function Counter({label}: {label: string}) {
  const [count, setCount] = useState(0);
  return (
    <View style={styles.row} testID="counter">
      <Text>{label}: {count}</Text>
      <Button title="Tap" onPress={() => { setCount(count + 1); console.log('tapped', count + 1); }} />
    </View>
  );
}

// Nested calls, so the stack of a crash has a few frames of this file to read.
function price(item: {cost?: {amount: number}}) {
  return item.cost!.amount;
}

function checkout() {
  return price({});
}

function Fruit({name}: {name: string}) {
  return <Text style={styles.item}>{name}</Text>;
}

export default function App() {
  const [enabled, setEnabled] = useState(true);
  return (
    <SafeAreaView style={styles.container}>
      <Text style={styles.title}>React Native demo</Text>
      <Counter label="Taps" />
      <Button title="Crash" onPress={() => { checkout(); }} />
      <View style={styles.row}>
        <Text>Notifications</Text>
        <Switch value={enabled} onValueChange={setEnabled} />
      </View>
      <FlatList
        data={['Apples', 'Bananas', 'Cherries', 'Dates']}
        keyExtractor={item => item}
        renderItem={({item}) => <Fruit name={item} />}
      />
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: {flex: 1, padding: 16},
  title: {fontSize: 24, fontWeight: '600', marginBottom: 12},
  row: {flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginVertical: 8},
  item: {paddingVertical: 8},
});
