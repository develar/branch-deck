package com.example.app

import com.example.shapes.Circle
import com.example.shapes.Shape as Figure
import com.example.util.*
import kotlinx.coroutines.flow.map as mapFlow

fun draw(figure: Figure) = Circle(1.0, "c", 1).describe() + figure.describe()
